//! Reactive application store: Leptos signals wrapped around the pure
//! domain logic in [`crate::state`].

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shakmaty::{san::SanPlus, Board, Chess, Color, Move, Position, Role, Square};
use wasm_bindgen_futures::JsFuture;

use crate::engine::{position_hash, Level, Search, MATE};
use crate::state::{
    click_moves, display_squares, game_status, parse_fen_position, parse_fen_setup, position_fen,
    setup_fen, setup_position, AppMode, Game, GameStatus, SetupTool,
};

/// Nodes searched per main-thread chunk before yielding to the browser.
const NODE_CHUNK: u64 = 50_000;
/// Minimum "thinking" time so instant replies still feel deliberate.
const MIN_THINK_MS: f64 = 350.0;
/// Fixed strength of the live-suggestion analysis (independent of the
/// opponent's level — advice should be good advice).
const ANALYSIS_LEVEL: Level = Level::Hard;

/// Which side(s) the engine controls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EngineSide {
    White,
    Black,
    /// Engine vs engine: it plays itself while you watch (or browse history).
    Both,
}

impl EngineSide {
    pub const ALL: [EngineSide; 3] = [EngineSide::White, EngineSide::Black, EngineSide::Both];

    pub fn covers(self, color: Color) -> bool {
        match self {
            EngineSide::White => color == Color::White,
            EngineSide::Black => color == Color::Black,
            EngineSide::Both => true,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EngineSide::White => "White",
            EngineSide::Black => "Black",
            EngineSide::Both => "Both",
        }
    }
}

/// A live engine recommendation for the displayed position.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Suggestion {
    pub from: Square,
    pub to: Square,
    pub san: String,
    /// Evaluation from White's perspective, e.g. "+0.4", "-1.2", "#3".
    pub eval: String,
}

/// Format a side-to-move score as a White-perspective eval string.
fn eval_text(score_stm: i32, turn: Color) -> String {
    let white = if turn.is_white() {
        score_stm
    } else {
        -score_stm
    };
    if white.abs() >= MATE - 200 {
        let moves = (MATE - white.abs() + 1) / 2;
        if white > 0 {
            format!("#{moves}")
        } else {
            format!("#-{moves}")
        }
    } else {
        format!("{:+.1}", f64::from(white) / 100.0)
    }
}

/// Engine opponent configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EngineConfig {
    pub enabled: bool,
    pub plays: EngineSide,
    pub level: Level,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            plays: EngineSide::Black,
            level: Level::Medium,
        }
    }
}

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
    /// Live best-move suggestions: analyse the displayed position in the
    /// background and show the engine's recommendation.
    pub show_suggestions: RwSignal<bool>,
    /// Latest analysis result for the displayed position.
    pub suggestion: RwSignal<Option<Suggestion>>,
    /// `true` while a suggestion analysis is in flight.
    pub analyzing: RwSignal<bool>,
    /// `search_gen` value that the current/last analysis was started for
    /// (prevents re-analysing an unchanged position).
    pub analyzed_gen: RwSignal<Option<u64>>,
    /// Engine opponent settings.
    pub engine: RwSignal<EngineConfig>,
    /// `true` while an engine search is in flight.
    pub thinking: RwSignal<bool>,
    /// Bumped whenever the game/engine state changes; in-flight searches
    /// compare against the value they started with and abort when stale.
    pub search_gen: RwSignal<u64>,
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
            show_suggestions: RwSignal::new(false),
            suggestion: RwSignal::new(None),
            analyzing: RwSignal::new(false),
            analyzed_gen: RwSignal::new(None),
            engine: RwSignal::new(EngineConfig::default()),
            thinking: RwSignal::new(false),
            search_gen: RwSignal::new(0),
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
        // The engine's pieces are not the human's to move.
        let engine = self.engine.get_untracked();
        if engine.enabled && engine.plays.covers(pos.turn()) {
            return;
        }
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
        // Any interaction that lands here changes what the engine should be
        // looking at, so invalidate in-flight searches.
        self.search_gen.update(|g| *g = g.wrapping_add(1));
    }

    // ------------------------------------------------------------------
    // Engine opponent
    // ------------------------------------------------------------------

    pub fn set_engine_enabled(&self, enabled: bool) {
        self.engine.update(|e| e.enabled = enabled);
        self.search_gen.update(|g| *g = g.wrapping_add(1));
    }

    pub fn set_engine_side(&self, side: EngineSide) {
        self.engine.update(|e| e.plays = side);
        self.search_gen.update(|g| *g = g.wrapping_add(1));
    }

    pub fn toggle_suggestions(&self) {
        self.show_suggestions.update(|s| *s = !*s);
        // Force a fresh analysis even if the position hasn't changed since
        // the toggle was last on.
        self.analyzed_gen.set(None);
    }

    pub fn set_engine_level(&self, level: Level) {
        self.engine.update(|e| e.level = level);
        self.search_gen.update(|g| *g = g.wrapping_add(1));
    }

    /// Tracked: `true` when the engine should start searching the latest
    /// position. Drives the effect installed in `App`.
    pub fn engine_should_move(&self) -> bool {
        let cfg = self.engine.get();
        cfg.enabled
            && !self.thinking.get()
            && self.mode.get() == AppMode::Play
            && self.game.with(|g| {
                g.at_latest() && {
                    let pos = &g.displayed().pos;
                    cfg.plays.covers(pos.turn())
                        // Engine-vs-engine games stop at the fifty-move rule
                        // instead of shuffling forever.
                        && pos.halfmoves() < 100
                        && matches!(
                            game_status(pos),
                            GameStatus::Turn(_) | GameStatus::Check(_)
                        )
                }
            })
    }

    /// Displayed position plus the Zobrist keys of the game positions that
    /// still matter for repetition detection (since the last irreversible
    /// move).
    fn snapshot_displayed(&self) -> (Chess, Vec<u64>) {
        self.game.with_untracked(|g| {
            let pos = g.displayed().pos.clone();
            let end = g.cursor() + 1;
            let start = end.saturating_sub(pos.halfmoves() as usize + 1);
            let prior: Vec<u64> = g.history()[start..end]
                .iter()
                .map(|ply| position_hash(&ply.pos))
                .collect();
            (pos, prior)
        })
    }

    /// Kick off an asynchronous, chunked engine search for the latest
    /// position. The search runs on the main thread but yields to the
    /// browser between node-budget chunks, so the UI stays responsive.
    pub fn spawn_engine_search(&self) {
        if self.thinking.get_untracked() {
            return;
        }
        let cfg = self.engine.get_untracked();
        let (pos, prior) = self.snapshot_displayed();
        self.thinking.set(true);
        let gen0 = self.search_gen.get_untracked();
        let store = *self;
        spawn_local(async move {
            let started = js_sys::Date::now();
            let seed = (started as u64) ^ gen0.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut search = Search::new(&pos, prior, cfg.level, seed);
            let deadline = started + cfg.level.time_budget_ms();
            loop {
                if store.search_gen.get_untracked() != gen0 {
                    // The user changed something; this search is stale.
                    store.thinking.set(false);
                    return;
                }
                if search.step(NODE_CHUNK) {
                    break;
                }
                if js_sys::Date::now() >= deadline {
                    break;
                }
                TimeoutFuture::new(0).await;
            }
            let best = search.take_best();
            let elapsed = js_sys::Date::now() - started;
            if elapsed < MIN_THINK_MS {
                TimeoutFuture::new((MIN_THINK_MS - elapsed) as u32).await;
            }
            if store.search_gen.get_untracked() == gen0 {
                if let Some(m) = best {
                    store.game.update(|g| {
                        g.play(m);
                    });
                    store.clear_transients();
                }
            }
            store.thinking.set(false);
        });
    }

    /// Tracked: `true` when a live suggestion should exist for the displayed
    /// position. Every viewed position qualifies (including history review)
    /// except the one the engine opponent is about to play itself — there its
    /// own search is already running.
    pub fn suggestions_wanted(&self) -> bool {
        self.show_suggestions.get() && self.mode.get() == AppMode::Play && {
            let cfg = self.engine.get();
            self.game.with(|g| {
                let pos = &g.displayed().pos;
                matches!(
                    game_status(pos),
                    GameStatus::Turn(_) | GameStatus::Check(_)
                ) && !(cfg.enabled && cfg.plays.covers(pos.turn()) && g.at_latest())
            })
        }
    }

    /// Analyse the displayed position in the background and publish the best
    /// move found so far into `suggestion` — refining live as the search
    /// deepens. Same chunked main-thread scheme as the opponent search.
    pub fn spawn_analysis(&self) {
        if self.analyzing.get_untracked() {
            return;
        }
        let (pos, prior) = self.snapshot_displayed();
        let gen0 = self.search_gen.get_untracked();
        self.analyzing.set(true);
        self.analyzed_gen.set(Some(gen0));
        self.suggestion.set(None);
        let store = *self;
        spawn_local(async move {
            let started = js_sys::Date::now();
            let seed = (started as u64) ^ gen0.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut search = Search::new(&pos, prior, ANALYSIS_LEVEL, seed);
            let deadline = started + ANALYSIS_LEVEL.time_budget_ms();
            loop {
                if store.search_gen.get_untracked() != gen0 {
                    break; // stale — a fresh analysis will be spawned
                }
                let done = search.step(NODE_CHUNK);
                if let Some((m, score, _)) = search.current_best_line() {
                    let (from, to) = display_squares(m);
                    let next = Suggestion {
                        from,
                        to,
                        san: SanPlus::from_move(pos.clone(), m).to_string(),
                        eval: eval_text(score, pos.turn()),
                    };
                    if store.search_gen.get_untracked() == gen0
                        && store.suggestion.get_untracked().as_ref() != Some(&next)
                    {
                        store.suggestion.set(Some(next));
                    }
                }
                if done || js_sys::Date::now() >= deadline {
                    break;
                }
                TimeoutFuture::new(0).await;
            }
            store.analyzing.set(false);
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_side_coverage() {
        assert!(EngineSide::White.covers(Color::White));
        assert!(!EngineSide::White.covers(Color::Black));
        assert!(EngineSide::Black.covers(Color::Black));
        assert!(!EngineSide::Black.covers(Color::White));
        assert!(EngineSide::Both.covers(Color::White));
        assert!(EngineSide::Both.covers(Color::Black));
    }
}
