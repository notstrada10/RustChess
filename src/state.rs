//! Pure game/domain logic for RustChess.
//!
//! Everything in this module is framework-free plain Rust on top of
//! [`shakmaty`], so it compiles natively and is covered by unit tests.

use shakmaty::{
    fen::Fen, san::SanPlus, Bitboard, Board, CastlingMode, Chess, Color, EnPassantMode, File,
    FromSetup, Move, Piece, Position, PositionError, PositionErrorKinds, Role, Setup, Square,
};

/// Top-level UI mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppMode {
    /// Normal alternating-turn play with full rules enforcement.
    Play,
    /// Free board editor: stamp, move and erase pieces at will.
    Setup,
}

/// Active tool while in [`AppMode::Setup`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupTool {
    /// Default tool: click a board piece to pick it up, click again to drop it.
    Pointer,
    /// One-shot carry of a piece picked up from `from` (drop cancels back to Pointer).
    Carry { piece: Piece, from: Square },
    /// Sticky stamp of a palette piece onto any square.
    Place(Piece),
    /// Remove pieces from clicked squares.
    Erase,
}

/// One entry of the game timeline: the position *after* `san` was played.
#[derive(Clone, Debug)]
pub struct Ply {
    pub pos: Chess,
    /// SAN of the move that led here (`None` for the root position).
    pub san: Option<String>,
    /// From/to squares to highlight (king from → king to for castling).
    pub last_move: Option<(Square, Square)>,
}

/// A navigable game timeline. Invariant: `history` is never empty and
/// `cursor < history.len()`.
#[derive(Clone, Debug)]
pub struct Game {
    history: Vec<Ply>,
    cursor: usize,
}

impl Game {
    pub fn new(root: Chess) -> Self {
        Self {
            history: vec![Ply {
                pos: root,
                san: None,
                last_move: None,
            }],
            cursor: 0,
        }
    }

    pub fn history(&self) -> &[Ply] {
        &self.history
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The ply currently shown on the board.
    pub fn displayed(&self) -> &Ply {
        &self.history[self.cursor]
    }

    pub fn at_start(&self) -> bool {
        self.cursor == 0
    }

    pub fn at_latest(&self) -> bool {
        self.cursor + 1 == self.history.len()
    }

    pub fn goto(&mut self, ply: usize) {
        self.cursor = ply.min(self.history.len() - 1);
    }

    pub fn first(&mut self) {
        self.cursor = 0;
    }

    pub fn prev(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn next(&mut self) {
        self.cursor = (self.cursor + 1).min(self.history.len() - 1);
    }

    pub fn last(&mut self) {
        self.cursor = self.history.len() - 1;
    }

    /// Play `m` from the *displayed* position. If the cursor sits in the middle
    /// of the timeline, all future plies are truncated first (standard
    /// branch-overwrites-future behaviour). Returns `false` for illegal moves.
    pub fn play(&mut self, m: Move) -> bool {
        let before = self.displayed().pos.clone();
        if !before.is_legal(m) {
            return false;
        }
        let san = SanPlus::from_move(before.clone(), m).to_string();
        match before.play(m) {
            Ok(after) => {
                self.history.truncate(self.cursor + 1);
                self.history.push(Ply {
                    pos: after,
                    san: Some(san),
                    last_move: Some(display_squares(m)),
                });
                self.cursor = self.history.len() - 1;
                true
            }
            Err(_) => false,
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new(Chess::default())
    }
}

/// The king's destination square for a castling move (standard chess).
fn castle_king_target(m: Move) -> Option<Square> {
    match (m, m.castling_side()) {
        (Move::Castle { king, .. }, Some(side)) => {
            Some(Square::from_coords(side.king_to_file(), king.rank()))
        }
        _ => None,
    }
}

/// Squares to highlight for a played move. Castling is normalised to the
/// king's real from → to squares (shakmaty encodes it as king → rook).
pub fn display_squares(m: Move) -> (Square, Square) {
    match m {
        Move::Castle { king, .. } => (king, castle_king_target(m).unwrap_or(king)),
        _ => (m.from().unwrap_or_else(|| m.to()), m.to()),
    }
}

/// All legal moves that a click from `from` to `to` should trigger.
/// More than one match means the user must pick a promotion piece.
/// Castling matches on the king's destination square (g/c-file).
pub fn click_moves(pos: &Chess, from: Square, to: Square) -> Vec<Move> {
    pos.legal_moves()
        .into_iter()
        .filter(|&m| match m {
            Move::Castle { king, .. } => king == from && castle_king_target(m) == Some(to),
            _ => m.from() == Some(from) && m.to() == to,
        })
        .collect()
}

/// `(all destinations, capture destinations)` for the piece on `from`,
/// used to render move hints.
pub fn destinations(pos: &Chess, from: Square) -> (Bitboard, Bitboard) {
    let mut all = Bitboard::EMPTY;
    let mut captures = Bitboard::EMPTY;
    for m in pos.legal_moves() {
        match m {
            Move::Castle { king, .. } if king == from => {
                if let Some(target) = castle_king_target(m) {
                    all.add(target);
                }
            }
            _ if m.from() == Some(from) => {
                all.add(m.to());
                if m.capture().is_some() {
                    captures.add(m.to());
                }
            }
            _ => {}
        }
    }
    (all, captures)
}

/// Promotion roles offered by a set of matching moves, in display order.
pub fn promotion_roles(moves: &[Move]) -> Vec<Role> {
    [Role::Queen, Role::Rook, Role::Bishop, Role::Knight]
        .into_iter()
        .filter(|&r| moves.iter().any(|m| m.promotion() == Some(r)))
        .collect()
}

/// Board-editor convention: grant castling rights exactly for rooks still on
/// their home corner squares while the matching king is on its home square.
pub fn detect_castling_rights(board: &Board) -> Bitboard {
    let mut rights = Bitboard::EMPTY;
    for color in [Color::White, Color::Black] {
        let back = color.backrank();
        if board.piece_at(Square::from_coords(File::E, back)) != Some(Role::King.of(color)) {
            continue;
        }
        for rook_file in [File::A, File::H] {
            let corner = Square::from_coords(rook_file, back);
            if board.piece_at(corner) == Some(Role::Rook.of(color)) {
                rights.add(corner);
            }
        }
    }
    rights
}

/// Build a full [`Setup`] from an arbitrary editor board + side to move.
fn make_setup(board: &Board, turn: Color) -> Setup {
    Setup {
        board: board.clone(),
        turn,
        castling_rights: detect_castling_rights(board),
        ..Setup::empty()
    }
}

/// FEN for an arbitrary (possibly illegal) editor position.
pub fn setup_fen(board: &Board, turn: Color) -> String {
    Fen::try_from_setup(make_setup(board, turn))
        .map(|fen| fen.to_string())
        .unwrap_or_else(|_| Fen::empty().to_string())
}

/// Try to promote an editor position into a playable [`Chess`] position,
/// relaxing the checks that a sandbox reasonably may (extra material,
/// physically impossible checks).
pub fn setup_position(board: &Board, turn: Color) -> Result<Chess, String> {
    Chess::from_setup(make_setup(board, turn), CastlingMode::Standard)
        .or_else(PositionError::ignore_too_much_material)
        .or_else(PositionError::ignore_impossible_check)
        .map_err(|e| friendly_position_error(&e))
}

/// Parse a pasted FEN into a playable position without ever panicking.
pub fn parse_fen_position(input: &str) -> Result<Chess, String> {
    let fen: Fen = input
        .trim()
        .parse()
        .map_err(|_| "Invalid FEN: could not parse the string".to_string())?;
    fen.into_position::<Chess>(CastlingMode::Standard)
        .or_else(PositionError::ignore_invalid_castling_rights)
        .or_else(PositionError::ignore_invalid_ep_square)
        .or_else(PositionError::ignore_too_much_material)
        .or_else(PositionError::ignore_impossible_check)
        .map_err(|e| friendly_position_error(&e))
}

/// Parse a pasted FEN into raw editor state (no legality requirements
/// beyond FEN syntax itself).
pub fn parse_fen_setup(input: &str) -> Result<(Board, Color), String> {
    let fen: Fen = input
        .trim()
        .parse()
        .map_err(|_| "Invalid FEN: could not parse the string".to_string())?;
    let setup = fen.into_setup();
    Ok((setup.board, setup.turn))
}

/// Canonical FEN of a live position.
pub fn position_fen(pos: &Chess) -> String {
    Fen::from_position(pos, EnPassantMode::Legal).to_string()
}

fn friendly_position_error(e: &PositionError<Chess>) -> String {
    let kinds = e.kinds();
    if kinds.contains(PositionErrorKinds::EMPTY_BOARD) {
        "The board is empty".to_string()
    } else if kinds.contains(PositionErrorKinds::MISSING_KING) {
        "Each side needs exactly one king".to_string()
    } else if kinds.contains(PositionErrorKinds::TOO_MANY_KINGS) {
        "Each side may only have one king".to_string()
    } else if kinds.contains(PositionErrorKinds::PAWNS_ON_BACKRANK) {
        "Pawns cannot stand on the first or eighth rank".to_string()
    } else if kinds.contains(PositionErrorKinds::OPPOSITE_CHECK) {
        "The side not to move is in check".to_string()
    } else {
        format!("Illegal position: {e}")
    }
}

/// Displayable game state of a position.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    /// Normal: given side to move.
    Turn(Color),
    /// Side to move is in check.
    Check(Color),
    /// Checkmate; the contained color is the *winner*.
    Checkmate(Color),
    Stalemate,
    /// Draw: neither side can possibly mate.
    InsufficientMaterial,
}

pub fn game_status(pos: &Chess) -> GameStatus {
    if pos.is_checkmate() {
        GameStatus::Checkmate(pos.turn().other())
    } else if pos.is_stalemate() {
        GameStatus::Stalemate
    } else if pos.is_insufficient_material() {
        GameStatus::InsufficientMaterial
    } else if pos.is_check() {
        GameStatus::Check(pos.turn())
    } else {
        GameStatus::Turn(pos.turn())
    }
}

/// One numbered row of the move list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MoveRow {
    pub number: u32,
    /// `(ply index, SAN)` of White's move, if present in this row.
    pub white: Option<(usize, String)>,
    /// `(ply index, SAN)` of Black's move, if present in this row.
    pub black: Option<(usize, String)>,
}

/// Fold the timeline into standard numbered SAN rows. Handles games whose
/// root position has Black to move (e.g. loaded from FEN) by leaving the
/// white half of the first row empty.
pub fn move_rows(history: &[Ply]) -> Vec<MoveRow> {
    let mut rows: Vec<MoveRow> = Vec::new();
    for (idx, ply) in history.iter().enumerate().skip(1) {
        let before = &history[idx - 1].pos;
        let number = before.fullmoves().get();
        let san = ply.san.clone().unwrap_or_default();
        if before.turn() == Color::White {
            rows.push(MoveRow {
                number,
                white: Some((idx, san)),
                black: None,
            });
        } else {
            match rows.last_mut() {
                Some(row) if row.number == number && row.black.is_none() && row.white.is_some() => {
                    row.black = Some((idx, san));
                }
                _ => rows.push(MoveRow {
                    number,
                    white: None,
                    black: Some((idx, san)),
                }),
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::san::San;

    const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    fn play_san(game: &mut Game, san: &str) {
        let pos = game.displayed().pos.clone();
        let m = san
            .parse::<San>()
            .expect("valid SAN")
            .to_move(&pos)
            .expect("legal move");
        assert!(game.play(m), "move {san} should be accepted");
    }

    #[test]
    fn starting_fen_round_trips() {
        let game = Game::default();
        assert_eq!(position_fen(&game.displayed().pos), START_FEN);
        let parsed = parse_fen_position(START_FEN).unwrap();
        assert_eq!(position_fen(&parsed), START_FEN);
    }

    #[test]
    fn plays_moves_and_records_san() {
        let mut game = Game::default();
        play_san(&mut game, "e4");
        play_san(&mut game, "e5");
        play_san(&mut game, "Nf3");
        assert_eq!(game.history().len(), 4);
        assert_eq!(game.cursor(), 3);
        let sans: Vec<_> = game
            .history()
            .iter()
            .filter_map(|p| p.san.clone())
            .collect();
        assert_eq!(sans, ["e4", "e5", "Nf3"]);
    }

    #[test]
    fn branching_truncates_the_future() {
        let mut game = Game::default();
        play_san(&mut game, "e4");
        play_san(&mut game, "e5");
        play_san(&mut game, "Nf3");
        game.goto(1); // back to the position after 1. e4
        play_san(&mut game, "c5"); // new branch: Sicilian
        assert_eq!(game.history().len(), 3);
        assert!(game.at_latest());
        assert_eq!(game.displayed().san.as_deref(), Some("c5"));
    }

    #[test]
    fn navigation_is_clamped() {
        let mut game = Game::default();
        play_san(&mut game, "d4");
        game.prev();
        game.prev();
        assert!(game.at_start());
        game.next();
        game.next();
        assert!(game.at_latest());
        game.goto(999);
        assert_eq!(game.cursor(), 1);
    }

    #[test]
    fn castling_is_matched_on_king_target_and_displayed_normalised() {
        let pos = parse_fen_position("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let short = click_moves(&pos, Square::E1, Square::G1);
        assert_eq!(short.len(), 1);
        assert_eq!(display_squares(short[0]), (Square::E1, Square::G1));
        let long = click_moves(&pos, Square::E1, Square::C1);
        assert_eq!(long.len(), 1);
        assert_eq!(display_squares(long[0]), (Square::E1, Square::C1));
    }

    #[test]
    fn promotion_clicks_offer_four_roles() {
        let pos = parse_fen_position("8/P7/8/8/8/8/k6K/8 w - - 0 1").unwrap();
        let moves = click_moves(&pos, Square::A7, Square::A8);
        assert_eq!(moves.len(), 4);
        assert_eq!(
            promotion_roles(&moves),
            vec![Role::Queen, Role::Rook, Role::Bishop, Role::Knight]
        );
    }

    #[test]
    fn fools_mate_is_checkmate_for_black() {
        let mut game = Game::default();
        for san in ["f3", "e5", "g4", "Qh4#"] {
            play_san(&mut game, san);
        }
        assert_eq!(
            game_status(&game.displayed().pos),
            GameStatus::Checkmate(Color::Black)
        );
        assert_eq!(game.displayed().san.as_deref(), Some("Qh4#"));
    }

    #[test]
    fn move_rows_handle_black_to_move_roots() {
        let root =
            parse_fen_position("rnbqkbnr/pppppppp/8/8/4P3/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1")
                .unwrap();
        let mut game = Game::new(root);
        play_san(&mut game, "e5");
        play_san(&mut game, "Nf3");
        let rows = move_rows(game.history());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].number, 1);
        assert!(rows[0].white.is_none());
        assert_eq!(rows[0].black.as_ref().unwrap().1, "e5");
        assert_eq!(rows[1].number, 2);
        assert_eq!(rows[1].white.as_ref().unwrap().1, "Nf3");
    }

    #[test]
    fn castling_rights_are_detected_from_home_squares() {
        let start = Board::new();
        let rights = detect_castling_rights(&start);
        for corner in [Square::A1, Square::H1, Square::A8, Square::H8] {
            assert!(rights.contains(corner));
        }
        // Move the white king off e1: white loses both rights.
        let mut board = Board::new();
        board.discard_piece_at(Square::E1);
        board.set_piece_at(Square::D3, Role::King.of(Color::White));
        let rights = detect_castling_rights(&board);
        assert!(!rights.contains(Square::A1));
        assert!(!rights.contains(Square::H1));
        assert!(rights.contains(Square::A8));
    }

    #[test]
    fn setup_fen_reflects_editor_state() {
        assert_eq!(
            setup_fen(&Board::empty(), Color::White),
            "8/8/8/8/8/8/8/8 w - - 0 1"
        );
        assert_eq!(setup_fen(&Board::new(), Color::White), START_FEN);
    }

    #[test]
    fn setup_position_rejects_missing_kings_with_friendly_message() {
        let err = setup_position(&Board::empty(), Color::White).unwrap_err();
        assert!(err.contains("king") || err.contains("empty"));
        let ok = setup_position(&Board::new(), Color::Black).unwrap();
        assert_eq!(ok.turn(), Color::Black);
    }

    #[test]
    fn invalid_fen_is_a_friendly_error() {
        assert!(parse_fen_position("definitely not a fen").is_err());
        assert!(parse_fen_setup("also/not/valid").is_err());
    }

    #[test]
    fn en_passant_and_capture_hints() {
        // After 1. e4 d5: the e4 pawn can capture on d5.
        let mut game = Game::default();
        play_san(&mut game, "e4");
        play_san(&mut game, "d5");
        let (all, captures) = destinations(&game.displayed().pos, Square::E4);
        assert!(all.contains(Square::E5));
        assert!(all.contains(Square::D5));
        assert!(captures.contains(Square::D5));
        assert!(!captures.contains(Square::E5));
    }
}
