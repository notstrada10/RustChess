//! Reactive application store: Leptos signals wrapped around the pure
//! domain logic in [`crate::state`].

use leptos::prelude::*;
use leptos::task::spawn_local;
use shakmaty::{Board, Chess, Color, Move, Position, Role, Square};
use wasm_bindgen_futures::JsFuture;

use crate::state::{
    click_moves, parse_fen_position, parse_fen_setup, position_fen, setup_fen, setup_position,
    AppMode, Game, SetupTool,
};

/// A pending pawn promotion waiting for the user to pick a piece.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PendingPromotion {
    pub from: Square,
    pub to: Square,
    pub color: Color,
}

/// Global app store. Every field is a `Copy` signal handle, so the whole
/// store is cheaply copyable into event handlers and provided via context.
#[derive(Clone, Copy)]
pub struct Store {
    pub mode: RwSignal<AppMode>,
    pub game: RwSignal<Game>,
    /// Currently selected square in Play mode.
    pub selected: RwSignal<Option<Square>>,
    /// Promotion picker state.
    pub promo: RwSignal<Option<PendingPromotion>>,
    /// Editor board (Setup mode).
    pub setup_board: RwSignal<Board>,
    /// Editor side to move (Setup mode).
    pub setup_turn: RwSignal<Color>,
    /// Active editor tool (Setup mode).
    pub tool: RwSignal<SetupTool>,
    /// Board orientation: `true` = Black at the bottom.
    pub flipped: RwSignal<bool>,
    /// Editable contents of the FEN input box.
    pub fen_draft: RwSignal<String>,
    /// Last user-facing error (FEN parse, illegal setup, ...).
    pub error: RwSignal<Option<String>>,
    /// Transient "copied to clipboard" indicator.
    pub copied: RwSignal<bool>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            mode: RwSignal::new(AppMode::Play),
            game: RwSignal::new(Game::default()),
            selected: RwSignal::new(None),
            promo: RwSignal::new(None),
            setup_board: RwSignal::new(Board::new()),
            setup_turn: RwSignal::new(Color::White),
            tool: RwSignal::new(SetupTool::Pointer),
            flipped: RwSignal::new(false),
            fen_draft: RwSignal::new(String::new()),
            error: RwSignal::new(None),
            copied: RwSignal::new(false),
        }
    }

    // ------------------------------------------------------------------
    // Derived state (tracked reads, for use inside memos/views)
    // ------------------------------------------------------------------

    /// FEN of whatever the board currently shows (play position or editor).
    pub fn current_fen(&self) -> String {
        match self.mode.get() {
            AppMode::Play => self.game.with(|g| position_fen(&g.displayed().pos)),
            AppMode::Setup => {
                let turn = self.setup_turn.get();
                self.setup_board.with(|b| setup_fen(b, turn))
            }
        }
    }

    /// Whether the editor position could be played as-is (tracked).
    pub fn setup_validity(&self) -> Result<(), String> {
        let turn = self.setup_turn.get();
        self.setup_board.with(|b| setup_position(b, turn).map(|_| ()))
    }

    // ------------------------------------------------------------------
    // Board interaction
    // ------------------------------------------------------------------

    pub fn click_square(&self, sq: Square) {
        match self.mode.get_untracked() {
            AppMode::Play => self.click_play(sq),
            AppMode::Setup => self.click_setup(sq),
        }
    }

    fn click_play(&self, sq: Square) {
        if self.promo.get_untracked().is_some() {
            return;
        }
        let pos = self.game.with_untracked(|g| g.displayed().pos.clone());
        let own_piece = pos
            .board()
            .piece_at(sq)
            .is_some_and(|p| p.color == pos.turn());
        match self.selected.get_untracked() {
            Some(from) if from == sq => self.selected.set(None),
            Some(from) => {
                let moves = click_moves(&pos, from, sq);
                if moves.is_empty() {
                    // Not a legal target: reselect own pieces, otherwise clear.
                    self.selected.set(own_piece.then_some(sq));
                } else if moves.iter().any(|m| m.promotion().is_some()) {
                    self.promo.set(Some(PendingPromotion {
                        from,
                        to: sq,
                        color: pos.turn(),
                    }));
                } else {
                    self.play_move(moves[0]);
                }
            }
            None if own_piece => self.selected.set(Some(sq)),
            None => {}
        }
    }

    fn click_setup(&self, sq: Square) {
        match self.tool.get_untracked() {
            SetupTool::Pointer => {
                if let Some(piece) = self.setup_board.with_untracked(|b| b.piece_at(sq)) {
                    self.tool.set(SetupTool::Carry { piece, from: sq });
                }
            }
            SetupTool::Carry { piece, from } => {
                if from != sq {
                    self.setup_board.update(|b| {
                        b.discard_piece_at(from);
                        b.set_piece_at(sq, piece);
                    });
                }
                self.tool.set(SetupTool::Pointer);
            }
            SetupTool::Place(piece) => {
                let same = self.setup_board.with_untracked(|b| b.piece_at(sq)) == Some(piece);
                self.setup_board.update(|b| {
                    if same {
                        b.discard_piece_at(sq);
                    } else {
                        b.set_piece_at(sq, piece);
                    }
                });
            }
            SetupTool::Erase => self.setup_board.update(|b| b.discard_piece_at(sq)),
        }
    }

    /// Right click: erase in Setup mode, clear the selection in Play mode.
    pub fn right_click_square(&self, sq: Square) {
        match self.mode.get_untracked() {
            AppMode::Setup => {
                if matches!(self.tool.get_untracked(), SetupTool::Carry { .. }) {
                    self.tool.set(SetupTool::Pointer);
                }
                self.setup_board.update(|b| b.discard_piece_at(sq));
            }
            AppMode::Play => self.selected.set(None),
        }
    }

    fn play_move(&self, m: Move) {
        self.game.update(|g| {
            g.play(m);
        });
        self.clear_transients();
        self.error.set(None);
    }

    pub fn choose_promotion(&self, role: Role) {
        let Some(pending) = self.promo.get_untracked() else {
            return;
        };
        let pos = self.game.with_untracked(|g| g.displayed().pos.clone());
        match click_moves(&pos, pending.from, pending.to)
            .into_iter()
            .find(|m| m.promotion() == Some(role))
        {
            Some(m) => self.play_move(m),
            None => self.promo.set(None),
        }
    }

    pub fn cancel_promotion(&self) {
        self.promo.set(None);
    }

    // ------------------------------------------------------------------
    // Timeline navigation
    // ------------------------------------------------------------------

    pub fn goto(&self, ply: usize) {
        self.game.update(|g| g.goto(ply));
        self.clear_transients();
    }

    pub fn nav_first(&self) {
        self.game.update(|g| g.first());
        self.clear_transients();
    }

    pub fn nav_prev(&self) {
        self.game.update(|g| g.prev());
        self.clear_transients();
    }

    pub fn nav_next(&self) {
        self.game.update(|g| g.next());
        self.clear_transients();
    }

    pub fn nav_last(&self) {
        self.game.update(|g| g.last());
        self.clear_transients();
    }

    // ------------------------------------------------------------------
    // Mode switching & setup actions
    // ------------------------------------------------------------------

    pub fn set_mode(&self, mode: AppMode) {
        if self.mode.get_untracked() == mode {
            return;
        }
        self.error.set(None);
        self.clear_transients();
        if mode == AppMode::Setup {
            // Seed the editor with whatever is currently displayed.
            let (board, turn) = self.game.with_untracked(|g| {
                let pos = &g.displayed().pos;
                (pos.board().clone(), pos.turn())
            });
            self.setup_board.set(board);
            self.setup_turn.set(turn);
            self.tool.set(SetupTool::Pointer);
        }
        self.mode.set(mode);
    }

    pub fn toggle_place_tool(&self, piece: shakmaty::Piece) {
        let current = self.tool.get_untracked();
        self.tool.set(if current == SetupTool::Place(piece) {
            SetupTool::Pointer
        } else {
            SetupTool::Place(piece)
        });
    }

    pub fn set_tool(&self, tool: SetupTool) {
        self.tool.set(tool);
    }

    pub fn clear_board(&self) {
        self.setup_board.set(Board::empty());
        self.tool.set(SetupTool::Pointer);
        self.error.set(None);
    }

    pub fn starting_position(&self) {
        self.setup_board.set(Board::new());
        self.setup_turn.set(Color::White);
        self.tool.set(SetupTool::Pointer);
        self.error.set(None);
    }

    /// Validate the editor position and, if legal, start a fresh game from it.
    pub fn play_from_setup(&self) {
        let turn = self.setup_turn.get_untracked();
        let result = self.setup_board.with_untracked(|b| setup_position(b, turn));
        match result {
            Ok(pos) => {
                self.game.set(Game::new(pos));
                self.mode.set(AppMode::Play);
                self.error.set(None);
                self.clear_transients();
            }
            Err(msg) => self.error.set(Some(msg)),
        }
    }

    pub fn new_game(&self) {
        self.game.set(Game::new(Chess::default()));
        self.mode.set(AppMode::Play);
        self.error.set(None);
        self.clear_transients();
    }

    pub fn flip(&self) {
        self.flipped.update(|f| *f = !*f);
    }

    /// Escape hatch: close the promotion picker, drop any carried piece,
    /// clear the selection.
    pub fn escape(&self) {
        if self.promo.get_untracked().is_some() {
            self.promo.set(None);
            return;
        }
        if self.mode.get_untracked() == AppMode::Setup
            && self.tool.get_untracked() != SetupTool::Pointer
        {
            self.tool.set(SetupTool::Pointer);
        }
        self.selected.set(None);
    }

    fn clear_transients(&self) {
        self.selected.set(None);
        self.promo.set(None);
    }

    // ------------------------------------------------------------------
    // FEN in/out
    // ------------------------------------------------------------------

    /// Apply the FEN input box to the board. In Play mode this starts a new
    /// game from the position; in Setup mode it just loads the pieces.
    pub fn load_fen(&self) {
        let text = self.fen_draft.get_untracked();
        match self.mode.get_untracked() {
            AppMode::Play => match parse_fen_position(&text) {
                Ok(pos) => {
                    self.game.set(Game::new(pos));
                    self.error.set(None);
                    self.clear_transients();
                }
                Err(msg) => self.error.set(Some(msg)),
            },
            AppMode::Setup => match parse_fen_setup(&text) {
                Ok((board, turn)) => {
                    self.setup_board.set(board);
                    self.setup_turn.set(turn);
                    self.tool.set(SetupTool::Pointer);
                    self.error.set(None);
                }
                Err(msg) => self.error.set(Some(msg)),
            },
        }
    }

    /// Copy the displayed FEN to the system clipboard and flash a confirmation.
    pub fn copy_fen(&self) {
        let fen = untrack(|| self.current_fen());
        let copied = self.copied;
        let clipboard = window().navigator().clipboard();
        spawn_local(async move {
            if JsFuture::from(clipboard.write_text(&fen)).await.is_ok() {
                copied.set(true);
                gloo_timers::future::TimeoutFuture::new(1500).await;
                copied.set(false);
            }
        });
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
