//! The 8×8 chessboard: fine-grained reactive squares over a CSS grid.

use leptos::prelude::*;
use shakmaty::{Bitboard, Board, File, Position, Rank, Square};

use super::{piece_asset, piece_label};
use crate::state::{destinations, AppMode, SetupTool};
use crate::store::Store;

/// Per-square overlay state. `PartialEq` lets each square's memo cut
/// re-renders down to exactly the squares that changed. The piece itself is
/// tracked in a separate memo so its DOM node (and its entrance animation)
/// is only re-created when the piece actually changes.
#[derive(Clone, Copy, PartialEq, Eq)]
struct SquareView {
    selected: bool,
    dest: bool,
    capture: bool,
    last_move: bool,
    check: bool,
}

/// Render order of the 64 squares for the current orientation.
fn square_order(flipped: bool) -> Vec<Square> {
    let mut order = Vec::with_capacity(64);
    for row in 0..8u32 {
        for col in 0..8u32 {
            let (file, rank) = if flipped {
                (File::new(7 - col), Rank::new(row))
            } else {
                (File::new(col), Rank::new(7 - row))
            };
            order.push(Square::from_coords(file, rank));
        }
    }
    order
}

#[component]
pub fn ChessBoard() -> impl IntoView {
    let store = expect_context::<Store>();

    // Board contents currently displayed (play position or editor board).
    let board = Memo::new(move |_| match store.mode.get() {
        AppMode::Play => store.game.with(|g| g.displayed().pos.board().clone()),
        AppMode::Setup => store.setup_board.get(),
    });

    // Move hints for the selected piece (Play mode only).
    let dests = Memo::new(move |_| -> (Bitboard, Bitboard) {
        if store.mode.get() != AppMode::Play {
            return (Bitboard::EMPTY, Bitboard::EMPTY);
        }
        match store.selected.get() {
            Some(from) => store.game.with(|g| destinations(&g.displayed().pos, from)),
            None => (Bitboard::EMPTY, Bitboard::EMPTY),
        }
    });

    // Last-move highlight (Play mode only).
    let last_move = Memo::new(move |_| match store.mode.get() {
        AppMode::Play => store.game.with(|g| g.displayed().last_move),
        AppMode::Setup => None,
    });

    // Square of a king currently in check (Play mode only).
    let check_sq = Memo::new(move |_| match store.mode.get() {
        AppMode::Play => store.game.with(|g| {
            let pos = &g.displayed().pos;
            if pos.is_check() {
                pos.board().king_of(pos.turn())
            } else {
                None
            }
        }),
        AppMode::Setup => None,
    });

    // Origin square of a piece being carried around in Setup mode.
    let carry_from = Memo::new(move |_| match store.tool.get() {
        SetupTool::Carry { from, .. } => Some(from),
        _ => None,
    });

    let order = Memo::new(move |_| square_order(store.flipped.get()));

    // Live engine suggestion, drawn as an arrow above the pieces.
    let arrow = Memo::new(move |_| -> Option<(Square, Square)> {
        if store.mode.get() != AppMode::Play || !store.show_suggestions.get() {
            return None;
        }
        store.suggestion.get().map(|s| (s.from, s.to))
    });

    view! {
        <div class="relative grid grid-cols-8 grid-rows-8 aspect-square w-full select-none overflow-hidden rounded-xl shadow-2xl shadow-slate-950/80 ring-1 ring-slate-700/60">
            <For each=move || order.get() key=|sq| *sq let:sq>
                <SquareCell sq=sq board=board dests=dests last_move=last_move check_sq=check_sq carry_from=carry_from />
            </For>
            {move || arrow.get().map(|(from, to)| suggestion_arrow(from, to, store.flipped.get()))}
        </div>
    }
}

/// Inline CSS that makes a freshly rendered piece slide in from the origin
/// square of the move that produced it (`None` = appear in place).
fn slide_style(sq: Square, last: Option<(Square, Square)>, flipped: bool) -> Option<String> {
    let (from, to) = last?;
    if to != sq || from == to {
        return None;
    }
    let (dx, dy) = if flipped {
        (
            (to.file() as i32 - from.file() as i32) * 100,
            (from.rank() as i32 - to.rank() as i32) * 100,
        )
    } else {
        (
            (from.file() as i32 - to.file() as i32) * 100,
            (to.rank() as i32 - from.rank() as i32) * 100,
        )
    };
    Some(format!(
        "--fx:{dx}%;--fy:{dy}%;animation:piece-slide 140ms ease-out;"
    ))
}

/// An arrow from `from` to `to` in board coordinates (1 unit = 1 square),
/// drawn lichess-style above the pieces.
fn suggestion_arrow(from: Square, to: Square, flipped: bool) -> impl IntoView {
    let center = |sq: Square| -> (f64, f64) {
        let file = f64::from(sq.file() as u32);
        let rank = f64::from(sq.rank() as u32);
        if flipped {
            (7.0 - file + 0.5, rank + 0.5)
        } else {
            (file + 0.5, 7.0 - rank + 0.5)
        }
    };
    let (x1, y1) = center(from);
    let (x2, y2) = center(to);
    let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt().max(0.001);
    let (ux, uy) = ((x2 - x1) / len, (y2 - y1) / len);
    let (px, py) = (-uy, ux); // perpendicular
    // Shaft: starts off the piece, stops where the head begins.
    let (sx, sy) = (x1 + ux * 0.32, y1 + uy * 0.32);
    let (ex, ey) = (x2 - ux * 0.36, y2 - uy * 0.36);
    // Head: triangle whose tip stops just short of the target's centre.
    let (tx, ty) = (x2 - ux * 0.05, y2 - uy * 0.05);
    let (b1x, b1y) = (ex + px * 0.24, ey + py * 0.24);
    let (b2x, b2y) = (ex - px * 0.24, ey - py * 0.24);
    view! {
        <svg viewBox="0 0 8 8" class="overlay-fade pointer-events-none absolute inset-0 h-full w-full">
            <path
                d=format!("M{sx:.3} {sy:.3} L{ex:.3} {ey:.3}")
                stroke="#38bdf8"
                stroke-width="0.18"
                stroke-linecap="round"
                fill="none"
                opacity="0.8"
            ></path>
            <path
                d=format!("M{tx:.3} {ty:.3} L{b1x:.3} {b1y:.3} L{b2x:.3} {b2y:.3} Z")
                fill="#38bdf8"
                opacity="0.8"
            ></path>
        </svg>
    }
}

#[component]
fn SquareCell(
    sq: Square,
    board: Memo<Board>,
    dests: Memo<(Bitboard, Bitboard)>,
    last_move: Memo<Option<(Square, Square)>>,
    check_sq: Memo<Option<Square>>,
    carry_from: Memo<Option<Square>>,
) -> impl IntoView {
    let store = expect_context::<Store>();

    let piece_mem = Memo::new(move |_| board.with(|b| b.piece_at(sq)));
    let carried_here = Memo::new(move |_| carry_from.get() == Some(sq));
    let view_state = Memo::new(move |_| {
        let (all, captures) = dests.get();
        SquareView {
            selected: store.selected.get() == Some(sq),
            dest: all.contains(sq),
            capture: captures.contains(sq),
            last_move: last_move
                .get()
                .is_some_and(|(from, to)| from == sq || to == sq),
            check: check_sq.get() == Some(sq),
        }
    });

    let is_dark = sq.is_dark();

    let cell_class = move || {
        let mut class = String::from("relative select-none ");
        class.push_str(if is_dark {
            "bg-emerald-800 "
        } else {
            "bg-slate-300 "
        });
        match store.mode.get() {
            AppMode::Play => class.push_str("cursor-pointer hover:brightness-110"),
            AppMode::Setup => match store.tool.get() {
                SetupTool::Pointer => class.push_str("cursor-pointer hover:brightness-110"),
                _ => class.push_str(
                    "cursor-crosshair hover:ring-2 hover:ring-inset hover:ring-emerald-400/70",
                ),
            },
        }
        class
    };

    // Coordinate labels live on the visual bottom row / left column.
    let file_label = move || {
        let bottom = if store.flipped.get() {
            Rank::Eighth
        } else {
            Rank::First
        };
        (sq.rank() == bottom).then(|| sq.file().char().to_string())
    };
    let rank_label = move || {
        let left = if store.flipped.get() {
            File::H
        } else {
            File::A
        };
        (sq.file() == left).then(|| sq.rank().char().to_string())
    };
    let label_class = if is_dark {
        "pointer-events-none absolute text-[9px] font-bold text-slate-300/70 sm:text-[11px]"
    } else {
        "pointer-events-none absolute text-[9px] font-bold text-emerald-900/70 sm:text-[11px]"
    };

    view! {
        <div
            class=cell_class
            on:click=move |_| store.click_square(sq)
            on:contextmenu=move |ev| {
                ev.prevent_default();
                store.right_click_square(sq);
            }
        >
            // --- overlays (below the piece) ---
            <Show when=move || view_state.get().last_move>
                <div class="overlay-fade pointer-events-none absolute inset-0 bg-amber-400/30"></div>
            </Show>
            <Show when=move || view_state.get().selected>
                <div class="overlay-fade pointer-events-none absolute inset-0 bg-sky-400/40"></div>
            </Show>
            <Show when=move || view_state.get().check>
                <div
                    class="overlay-fade pointer-events-none absolute inset-0"
                    style="background: radial-gradient(circle, rgba(244,63,94,0.85) 0%, rgba(244,63,94,0.35) 55%, rgba(244,63,94,0) 80%)"
                ></div>
            </Show>
            <Show when=move || view_state.get().capture>
                <div
                    data-hint="capture"
                    class="overlay-fade pointer-events-none absolute inset-[4%] rounded-full border-4 border-emerald-400/70 sm:border-[5px]"
                ></div>
            </Show>

            // --- coordinates ---
            {move || file_label().map(|f| view! { <span class=label_class style="right:3px;bottom:1px">{f}</span> })}
            {move || rank_label().map(|r| view! { <span class=label_class style="left:3px;top:1px">{r}</span> })}

            // --- the piece (re-created only when the piece itself changes,
            //     sliding in from its origin square for played moves) ---
            {move || {
                piece_mem
                    .get()
                    .map(|p| {
                        let slide = untrack(|| {
                            slide_style(sq, last_move.get_untracked(), store.flipped.get_untracked())
                        });
                        let img_class = move || {
                            if carried_here.get() {
                                "pointer-events-none absolute inset-0 h-full w-full p-[4%] opacity-40"
                            } else {
                                "pointer-events-none absolute inset-0 h-full w-full p-[4%] drop-shadow-sm"
                            }
                        };
                        view! {
                            <img
                                src=piece_asset(p)
                                alt=piece_label(p)
                                draggable="false"
                                class=img_class
                                style=slide
                            />
                        }
                    })
            }}

            // --- quiet-move hint dot (above empty squares) ---
            <Show when=move || {
                let vs = view_state.get();
                vs.dest && !vs.capture
            }>
                <div class="overlay-fade pointer-events-none absolute inset-0 flex items-center justify-center">
                    <div data-hint="dot" class="h-[28%] w-[28%] rounded-full bg-emerald-950/40"></div>
                </div>
            </Show>
        </div>
    }
}
