//! A small but real chess engine, 100% Rust, running inside the app's own
//! WASM binary.
//!
//! Search: iterative-deepening negamax with alpha-beta pruning, quiescence
//! search, a transposition table, MVV-LVA + killer-move ordering and check
//! extensions. Evaluation: material + piece-square tables (Michniewski's
//! "simplified evaluation function") with a phase-tapered king table.
//!
//! The search is driven through [`Search::step`] in bounded node-count
//! chunks so the UI thread can yield between chunks and never freezes.

use shakmaty::zobrist::Zobrist64;
use shakmaty::{Chess, EnPassantMode, Move, MoveList, Position, Role};

pub const MATE: i32 = 30_000;
const INF: i32 = 31_000;
const MAX_PLY: usize = 64;
const TT_SIZE: usize = 1 << 17; // 131k entries

/// Engine strength presets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Beginner,
    Easy,
    Medium,
    Hard,
    Max,
}

impl Level {
    pub const ALL: [Level; 5] = [
        Level::Beginner,
        Level::Easy,
        Level::Medium,
        Level::Hard,
        Level::Max,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Level::Beginner => "Beginner",
            Level::Easy => "Easy",
            Level::Medium => "Medium",
            Level::Hard => "Hard",
            Level::Max => "Max",
        }
    }

    /// Maximum iterative-deepening depth.
    fn max_depth(self) -> u8 {
        match self {
            Level::Beginner => 1,
            Level::Easy => 2,
            Level::Medium => 4,
            Level::Hard => 6,
            Level::Max => 18,
        }
    }

    /// Wall-clock budget enforced by the async driver.
    pub fn time_budget_ms(self) -> f64 {
        match self {
            Level::Beginner => 150.0,
            Level::Easy => 250.0,
            Level::Medium => 700.0,
            Level::Hard => 1400.0,
            Level::Max => 2800.0,
        }
    }

    /// Sloppiness: pick randomly among root moves within this margin
    /// (centipawns) of the best. 0 = always the best move found.
    fn noise_cp(self) -> i32 {
        match self {
            Level::Beginner => 150,
            Level::Easy => 75,
            _ => 0,
        }
    }
}

/// Stable u64 hash of a position (Zobrist).
pub fn position_hash(pos: &Chess) -> u64 {
    pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

fn role_index(role: Role) -> usize {
    match role {
        Role::Pawn => 0,
        Role::Knight => 1,
        Role::Bishop => 2,
        Role::Rook => 3,
        Role::Queen => 4,
        Role::King => 5,
    }
}

/// Material in centipawns (king excluded from material sums).
const MATERIAL: [i32; 6] = [100, 320, 330, 500, 900, 0];

/// Small per-role weights for MVV-LVA capture ordering.
const ORDER_WEIGHT: [i32; 6] = [1, 3, 3, 5, 9, 20];

/// Piece-square tables, written rank 8 first (as seen from White's side).
/// Indexed by `role_index` for P, N, B, R, Q and the middlegame king.
#[rustfmt::skip]
const PST: [[i32; 64]; 6] = [
    // Pawn
    [
         0,  0,  0,  0,  0,  0,  0,  0,
        50, 50, 50, 50, 50, 50, 50, 50,
        10, 10, 20, 30, 30, 20, 10, 10,
         5,  5, 10, 25, 25, 10,  5,  5,
         0,  0,  0, 20, 20,  0,  0,  0,
         5, -5,-10,  0,  0,-10, -5,  5,
         5, 10, 10,-20,-20, 10, 10,  5,
         0,  0,  0,  0,  0,  0,  0,  0,
    ],
    // Knight
    [
       -50,-40,-30,-30,-30,-30,-40,-50,
       -40,-20,  0,  0,  0,  0,-20,-40,
       -30,  0, 10, 15, 15, 10,  0,-30,
       -30,  5, 15, 20, 20, 15,  5,-30,
       -30,  0, 15, 20, 20, 15,  0,-30,
       -30,  5, 10, 15, 15, 10,  5,-30,
       -40,-20,  0,  5,  5,  0,-20,-40,
       -50,-40,-30,-30,-30,-30,-40,-50,
    ],
    // Bishop
    [
       -20,-10,-10,-10,-10,-10,-10,-20,
       -10,  0,  0,  0,  0,  0,  0,-10,
       -10,  0,  5, 10, 10,  5,  0,-10,
       -10,  5,  5, 10, 10,  5,  5,-10,
       -10,  0, 10, 10, 10, 10,  0,-10,
       -10, 10, 10, 10, 10, 10, 10,-10,
       -10,  5,  0,  0,  0,  0,  5,-10,
       -20,-10,-10,-10,-10,-10,-10,-20,
    ],
    // Rook
    [
         0,  0,  0,  0,  0,  0,  0,  0,
         5, 10, 10, 10, 10, 10, 10,  5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
         0,  0,  0,  5,  5,  0,  0,  0,
    ],
    // Queen
    [
       -20,-10,-10, -5, -5,-10,-10,-20,
       -10,  0,  0,  0,  0,  0,  0,-10,
       -10,  0,  5,  5,  5,  5,  0,-10,
        -5,  0,  5,  5,  5,  5,  0, -5,
         0,  0,  5,  5,  5,  5,  0, -5,
       -10,  5,  5,  5,  5,  5,  0,-10,
       -10,  0,  5,  0,  0,  0,  0,-10,
       -20,-10,-10, -5, -5,-10,-10,-20,
    ],
    // King (middlegame)
    [
       -30,-40,-40,-50,-50,-40,-40,-30,
       -30,-40,-40,-50,-50,-40,-40,-30,
       -30,-40,-40,-50,-50,-40,-40,-30,
       -30,-40,-40,-50,-50,-40,-40,-30,
       -20,-30,-30,-40,-40,-30,-30,-20,
       -10,-20,-20,-20,-20,-20,-20,-10,
        20, 20,  0,  0,  0,  0, 20, 20,
        20, 30, 10,  0,  0, 10, 30, 20,
    ],
];

/// King endgame table (centralise the king once material comes off).
#[rustfmt::skip]
const KING_EG: [i32; 64] = [
   -50,-40,-30,-20,-20,-30,-40,-50,
   -30,-20,-10,  0,  0,-10,-20,-30,
   -30,-10, 20, 30, 30, 20,-10,-30,
   -30,-10, 30, 40, 40, 30,-10,-30,
   -30,-10, 30, 40, 40, 30,-10,-30,
   -30,-10, 20, 30, 30, 20,-10,-30,
   -30,-30,  0,  0,  0,  0,-30,-30,
   -50,-30,-30,-30,-30,-30,-30,-50,
];

/// Game phase in [0, 24]: 24 = full middlegame material, 0 = bare endgame.
fn game_phase(board: &shakmaty::Board) -> i32 {
    let phase = board.knights().count() as i32
        + board.bishops().count() as i32
        + 2 * board.rooks().count() as i32
        + 4 * board.queens().count() as i32;
    phase.min(24)
}

/// Static evaluation in centipawns from the side to move's perspective.
fn evaluate(pos: &Chess) -> i32 {
    let board = pos.board();
    let phase = game_phase(board);
    let mut white = 0i32;
    for sq in board.occupied() {
        let Some(piece) = board.piece_at(sq) else {
            continue;
        };
        let role = role_index(piece.role);
        // Tables are stored rank-8-first; White reads them flipped.
        let (file, rank) = (sq.file() as usize, sq.rank() as usize);
        let idx_white = (7 - rank) * 8 + file;
        let idx_black = rank * 8 + file;
        let idx = if piece.color.is_white() {
            idx_white
        } else {
            idx_black
        };
        let pst = if piece.role == Role::King {
            // Taper the king between its middlegame and endgame tables.
            (PST[5][idx] * phase + KING_EG[idx] * (24 - phase)) / 24
        } else {
            PST[role][idx]
        };
        let value = MATERIAL[role] + pst;
        if piece.color.is_white() {
            white += value;
        } else {
            white -= value;
        }
    }
    if pos.turn().is_white() {
        white
    } else {
        -white
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

const FLAG_EXACT: u8 = 0;
const FLAG_LOWER: u8 = 1;
const FLAG_UPPER: u8 = 2;

#[derive(Clone, Copy)]
struct TtEntry {
    key: u64,
    depth: i8,
    flag: u8,
    score: i32,
    best: Option<Move>,
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// MVV-LVA-ish ordering score for a move.
fn order_score(m: Move, tt_move: Option<Move>, killers: &[Option<Move>; 2]) -> i32 {
    if Some(m) == tt_move {
        return 1_000_000;
    }
    if let Some(victim) = m.capture() {
        let attacker = ORDER_WEIGHT[role_index(m.role())];
        return 100_000 + ORDER_WEIGHT[role_index(victim)] * 16 - attacker;
    }
    if m.promotion() == Some(Role::Queen) {
        return 90_000;
    }
    if killers.contains(&Some(m)) {
        return 80_000;
    }
    0
}

/// A chunked, resumable game-tree search for one position.
pub struct Search {
    root: Chess,
    max_depth: u8,
    noise: i32,
    rng: u64,
    /// Zobrist keys of the actual game so far (draw-by-repetition awareness).
    prior: Vec<u64>,
    /// Zobrist keys along the current search line.
    path: Vec<u64>,
    tt: Vec<Option<TtEntry>>,
    killers: [[Option<Move>; 2]; MAX_PLY],
    nodes: u64,
    node_limit: u64,
    aborted: bool,
    // Iterative-deepening state.
    depth: u8,
    root_moves: Vec<Move>,
    iter_scores: Vec<Option<i32>>,
    iter_best: i32,
    idx: usize,
    retries: u32,
    /// Scores of the last fully completed iteration, sorted best-first.
    last_scores: Vec<(Move, i32)>,
    depth_completed: u8,
    chosen: Option<Move>,
    done: bool,
}

impl Search {
    pub fn new(pos: &Chess, prior_hashes: Vec<u64>, level: Level, seed: u64) -> Self {
        let mut root_moves: Vec<Move> = pos.legal_moves().into_iter().collect();
        // Cheap first-iteration ordering: captures and promotions up front.
        root_moves.sort_by_key(|&m| -order_score(m, None, &[None, None]));
        let mut search = Self {
            root: pos.clone(),
            max_depth: level.max_depth(),
            noise: level.noise_cp(),
            rng: seed | 1,
            prior: prior_hashes,
            path: Vec::with_capacity(MAX_PLY + 8),
            tt: vec![None; TT_SIZE],
            killers: [[None; 2]; MAX_PLY],
            nodes: 0,
            node_limit: 0,
            aborted: false,
            depth: 1,
            iter_scores: vec![None; root_moves.len()],
            iter_best: -INF,
            idx: 0,
            retries: 0,
            last_scores: Vec::new(),
            depth_completed: 0,
            chosen: None,
            done: false,
            root_moves,
        };
        // Trivial positions need no search at all.
        match search.root_moves.len() {
            0 => search.done = true,
            1 => {
                search.chosen = Some(search.root_moves[0]);
                search.done = true;
            }
            _ => {}
        }
        search
    }

    // Search statistics — exercised by the test suite; handy for a future
    // eval-bar / analysis UI.
    #[allow(dead_code)]
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    #[allow(dead_code)]
    pub fn depth_completed(&self) -> u8 {
        self.depth_completed
    }

    /// Best score (centipawns, side to move) of the last completed iteration.
    #[allow(dead_code)]
    pub fn score(&self) -> Option<i32> {
        self.last_scores.first().map(|&(_, s)| s)
    }

    /// Best `(move, score, depth)` found so far, readable mid-search without
    /// finalizing — powers the live analysis display.
    pub fn current_best_line(&self) -> Option<(Move, i32, u8)> {
        self.last_scores
            .first()
            .map(|&(m, s)| (m, s, self.depth_completed))
    }

    /// Run roughly `node_budget` more nodes. Returns `true` once the search
    /// has finished (max depth reached, mate found, or trivial position).
    pub fn step(&mut self, node_budget: u64) -> bool {
        if self.done {
            return true;
        }
        self.node_limit = self.nodes.saturating_add(node_budget);
        self.aborted = false;

        while !self.done && self.nodes < self.node_limit {
            let m = self.root_moves[self.idx];
            let mut child = self.root.clone();
            child.play_unchecked(m);
            self.path.clear();
            // Full-window search at noisy levels so every root score is exact
            // (needed for the "pick within a margin" selection).
            let alpha = if self.noise > 0 { -INF } else { self.iter_best };
            let score = -self.alpha_beta(&child, i32::from(self.depth) - 1, -INF, -alpha, 1);

            if self.aborted {
                // The chunk budget ran out mid-move. The transposition table
                // kept the work, so retrying this root move next step is
                // cheap. Bail out entirely if it keeps happening.
                self.retries += 1;
                if self.retries > 6 {
                    self.finalize();
                }
                return self.done;
            }
            self.retries = 0;
            self.iter_scores[self.idx] = Some(score);
            if score > self.iter_best {
                self.iter_best = score;
            }
            self.idx += 1;

            if self.idx == self.root_moves.len() {
                self.complete_iteration();
            }
        }
        self.done
    }

    /// Final best move; finalizes a timed-out search on the fly.
    pub fn take_best(&mut self) -> Option<Move> {
        if !self.done {
            self.finalize();
        }
        self.chosen
    }

    fn complete_iteration(&mut self) {
        let mut paired: Vec<(Move, i32)> = self
            .root_moves
            .iter()
            .zip(self.iter_scores.iter())
            .filter_map(|(&m, s)| s.map(|s| (m, s)))
            .collect();
        paired.sort_by_key(|&(_, s)| -s);
        self.last_scores = paired;
        self.depth_completed = self.depth;
        self.root_moves = self.last_scores.iter().map(|&(m, _)| m).collect();

        let best = self.last_scores.first().map_or(-INF, |&(_, s)| s);
        if self.depth >= self.max_depth || best >= MATE - 200 {
            self.finalize();
        } else {
            self.depth += 1;
            self.idx = 0;
            self.iter_best = -INF;
            self.iter_scores = vec![None; self.root_moves.len()];
        }
    }

    fn finalize(&mut self) {
        if self.chosen.is_none() {
            // Prefer results of the freshest iteration; fall back to partial
            // scores of the current one, then to the move ordering itself.
            let pool: Vec<(Move, i32)> = if !self.last_scores.is_empty() {
                self.last_scores.clone()
            } else {
                let mut partial: Vec<(Move, i32)> = self
                    .root_moves
                    .iter()
                    .zip(self.iter_scores.iter())
                    .filter_map(|(&m, s)| s.map(|s| (m, s)))
                    .collect();
                partial.sort_by_key(|&(_, s)| -s);
                partial
            };
            self.chosen = if pool.is_empty() {
                self.root_moves.first().copied()
            } else if self.noise > 0 {
                let best = pool[0].1;
                let cands: Vec<Move> = pool
                    .iter()
                    .filter(|&&(_, s)| s >= best - self.noise)
                    .map(|&(m, _)| m)
                    .collect();
                let pick = (xorshift(&mut self.rng) % cands.len() as u64) as usize;
                Some(cands[pick])
            } else {
                Some(pool[0].0)
            };
        }
        self.done = true;
    }

    fn alpha_beta(&mut self, pos: &Chess, depth: i32, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;
        if self.nodes > self.node_limit {
            self.aborted = true;
            return 0;
        }
        if ply >= MAX_PLY {
            return evaluate(pos);
        }

        // Draw detection: fifty-move rule, dead material, repetition of any
        // position along the search line or the played game.
        if pos.halfmoves() >= 100 || pos.is_insufficient_material() {
            return 0;
        }
        let key = position_hash(pos);
        let relevant = pos.halfmoves() as usize;
        if self
            .path
            .iter()
            .rev()
            .take(relevant)
            .chain(self.prior.iter().rev().take(relevant.saturating_sub(self.path.len())))
            .any(|&k| k == key)
        {
            return 0;
        }

        let in_check = pos.is_check();
        let depth = if in_check { depth + 1 } else { depth }; // check extension
        if depth <= 0 {
            return self.quiescence(pos, alpha, beta, ply);
        }

        // Transposition table probe.
        let tt_idx = (key as usize) & (TT_SIZE - 1);
        let mut tt_move = None;
        if let Some(entry) = &self.tt[tt_idx] {
            if entry.key == key {
                tt_move = entry.best;
                if i32::from(entry.depth) >= depth {
                    match entry.flag {
                        FLAG_EXACT => return entry.score,
                        FLAG_LOWER if entry.score >= beta => return entry.score,
                        FLAG_UPPER if entry.score <= alpha => return entry.score,
                        _ => {}
                    }
                }
            }
        }

        let mut moves = pos.legal_moves();
        if moves.is_empty() {
            return if in_check { -(MATE - ply as i32) } else { 0 };
        }
        let killers = self.killers[ply];
        moves.sort_by_key(|&m| -order_score(m, tt_move, &killers));

        let alpha_orig = alpha;
        let mut best_score = -INF;
        let mut best_move = None;
        self.path.push(key);
        for &m in &moves {
            let mut child = pos.clone();
            child.play_unchecked(m);
            let score = -self.alpha_beta(&child, depth - 1, -beta, -alpha, ply + 1);
            if self.aborted {
                self.path.pop();
                return 0;
            }
            if score > best_score {
                best_score = score;
                best_move = Some(m);
                if score > alpha {
                    alpha = score;
                }
            }
            if alpha >= beta {
                // Remember quiet cutoff moves as killers for this ply.
                if m.capture().is_none() && m.promotion().is_none() {
                    let ks = &mut self.killers[ply];
                    if ks[0] != Some(m) {
                        ks[1] = ks[0];
                        ks[0] = Some(m);
                    }
                }
                break;
            }
        }
        self.path.pop();

        // Store in the TT (skip unstable mate-range scores).
        if best_score.abs() < MATE - 500 {
            let flag = if best_score <= alpha_orig {
                FLAG_UPPER
            } else if best_score >= beta {
                FLAG_LOWER
            } else {
                FLAG_EXACT
            };
            self.tt[tt_idx] = Some(TtEntry {
                key,
                depth: depth as i8,
                flag,
                score: best_score,
                best: best_move,
            });
        }
        best_score
    }

    fn quiescence(&mut self, pos: &Chess, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;
        if self.nodes > self.node_limit {
            self.aborted = true;
            return 0;
        }
        if ply >= MAX_PLY {
            return evaluate(pos);
        }

        let in_check = pos.is_check();
        let mut moves: MoveList;
        let mut best;
        if in_check {
            // In check: search every evasion (stand-pat would be illegal).
            moves = pos.legal_moves();
            if moves.is_empty() {
                return -(MATE - ply as i32);
            }
            best = -INF;
        } else {
            let stand_pat = evaluate(pos);
            if stand_pat >= beta {
                return stand_pat;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
            best = stand_pat;
            moves = pos.capture_moves();
        }
        moves.sort_by_key(|&m| -order_score(m, None, &[None, None]));

        for &m in &moves {
            let mut child = pos.clone();
            child.play_unchecked(m);
            let score = -self.quiescence(&child, -beta, -alpha, ply + 1);
            if self.aborted {
                return 0;
            }
            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                }
            }
            if alpha >= beta {
                break;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::parse_fen_position;
    use shakmaty::Square;

    /// Drive a search to completion the way the async driver does.
    fn best_move(fen: &str, level: Level) -> (Option<Move>, Search) {
        let pos = parse_fen_position(fen).expect("valid test FEN");
        let mut search = Search::new(&pos, Vec::new(), level, 0xC0FFEE);
        let mut steps = 0;
        while !search.step(200_000) {
            steps += 1;
            assert!(steps < 500, "search failed to converge");
        }
        (search.take_best(), search)
    }

    fn san_of(fen: &str, m: Move) -> String {
        let pos = parse_fen_position(fen).unwrap();
        shakmaty::san::SanPlus::from_move(pos, m).to_string()
    }

    #[test]
    fn finds_mate_in_one() {
        let fen = "6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1";
        let (best, search) = best_move(fen, Level::Medium);
        assert_eq!(san_of(fen, best.unwrap()), "Ra8#");
        assert!(search.score().unwrap() >= MATE - 100);
    }

    #[test]
    fn finds_forced_mate_in_two() {
        // Ladder mate: 1. Rb7+ Kg8 2. Ra8#
        let fen = "8/6k1/R7/1R6/8/8/8/K7 w - - 0 1";
        let (best, search) = best_move(fen, Level::Hard);
        assert!(search.score().unwrap() >= MATE - 100, "should see the mate");
        let san = san_of(fen, best.unwrap());
        assert!(san.starts_with("Rb7+") || san.starts_with("Ra7+"), "got {san}");
    }

    #[test]
    fn grabs_a_hanging_queen() {
        let fen = "k7/8/8/3q4/4P3/8/8/7K w - - 0 1";
        let (best, _) = best_move(fen, Level::Easy);
        assert_eq!(best.unwrap().to(), Square::D5, "must capture the queen");
    }

    #[test]
    fn single_legal_move_returns_instantly() {
        // White king a1 boxed in by the f2 queen: only Kb1 is legal.
        let fen = "k7/8/8/8/8/8/5q2/K7 w - - 0 1";
        let pos = parse_fen_position(fen).unwrap();
        assert_eq!(pos.legal_moves().len(), 1);
        let mut search = Search::new(&pos, Vec::new(), Level::Max, 1);
        assert!(search.step(1), "should be done without searching");
        assert!(search.take_best().is_some());
    }

    #[test]
    fn beginner_noise_still_plays_legal_moves() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let pos = parse_fen_position(fen).unwrap();
        for seed in 1..6u64 {
            let mut search = Search::new(&pos, Vec::new(), Level::Beginner, seed);
            while !search.step(50_000) {}
            let m = search.take_best().unwrap();
            assert!(pos.is_legal(m), "noisy move must still be legal");
        }
    }

    #[test]
    fn does_not_walk_into_repetition_when_winning() {
        // Up a queen: the best line should never be scored as a draw-by-
        // repetition shuffle. Just sanity-check score stays clearly winning.
        let fen = "k7/8/8/8/8/8/8/KQ6 w - - 0 1";
        let (_, search) = best_move(fen, Level::Medium);
        assert!(search.score().unwrap() > 500);
    }

    #[test]
    fn chunked_stepping_matches_time_slicing() {
        // Many tiny steps must converge to a finished search.
        let fen = "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 3";
        let pos = parse_fen_position(fen).unwrap();
        let mut search = Search::new(&pos, Vec::new(), Level::Medium, 42);
        let mut steps = 0;
        while !search.step(5_000) {
            steps += 1;
            assert!(steps < 2_000, "must converge with tiny chunks");
        }
        assert!(search.take_best().is_some());
        assert!(search.depth_completed() >= 3);
        assert!(search.nodes() > 5_000, "should have searched a real tree");
    }

    #[test]
    fn evaluation_is_symmetric() {
        let start = parse_fen_position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .unwrap();
        assert_eq!(evaluate(&start), 0, "start position must be balanced");
        let mirrored =
            parse_fen_position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1").unwrap();
        assert_eq!(evaluate(&start), evaluate(&mirrored));
    }
}
