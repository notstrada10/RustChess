//! The 8×8 chessboard: fine-grained reactive squares over a CSS grid.

use leptos::prelude::*;
use shakmaty::{Bitboard, Board, File, Piece, Position, Rank, Square};

use super::{piece_asset, piece_label};
use crate::state::{destinations, AppMode, SetupTool};
use crate::store::Store;

/// Per-square derived render state. `PartialEq` lets each square's memo cut
/// re-renders down to exactly the squares that changed.
#[derive(Clone, Copy, PartialEq, Eq)]
struct SquareView {
    piece: Option<Piece>,
    selected: bool,
    dest: bool,
    capture: bool,
    last_move: bool,
    check: bool,
    carried: bool,
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

    view! {
        <div class="grid grid-cols-8 grid-rows-8 aspect-square w-full select-none overflow-hidden rounded-xl shadow-2xl shadow-slate-950/80 ring-1 ring-slate-700/60">
            <For each=move || order.get() key=|sq| *sq let:sq>
                <SquareCell sq=sq board=board dests=dests last_move=last_move check_sq=check_sq carry_from=carry_from />
            </For>
        </div>
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

    let view_state = Memo::new(move |_| {
        let (all, captures) = dests.get();
        SquareView {
            piece: board.with(|b| b.piece_at(sq)),
            selected: store.selected.get() == Some(sq),
            dest: all.contains(sq),
            capture: captures.contains(sq),
            last_move: last_move
                .get()
                .is_some_and(|(from, to)| from == sq || to == sq),
            check: check_sq.get() == Some(sq),
            carried: carry_from.get() == Some(sq),
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
                <div class="pointer-events-none absolute inset-0 bg-amber-400/30"></div>
            </Show>
            <Show when=move || view_state.get().selected>
                <div class="pointer-events-none absolute inset-0 bg-sky-400/40"></div>
            </Show>
            <Show when=move || view_state.get().check>
                <div
                    class="pointer-events-none absolute inset-0"
                    style="background: radial-gradient(circle, rgba(244,63,94,0.85) 0%, rgba(244,63,94,0.35) 55%, rgba(244,63,94,0) 80%)"
                ></div>
            </Show>
            <Show when=move || view_state.get().capture>
                <div class="pointer-events-none absolute inset-[4%] rounded-full border-4 border-emerald-400/70 sm:border-[5px]"></div>
            </Show>

            // --- coordinates ---
            {move || file_label().map(|f| view! { <span class=label_class style="right:3px;bottom:1px">{f}</span> })}
            {move || rank_label().map(|r| view! { <span class=label_class style="left:3px;top:1px">{r}</span> })}

            // --- the piece ---
            {move || {
                let vs = view_state.get();
                vs.piece
                    .map(|p| {
                        let img_class = if vs.carried {
                            "pointer-events-none absolute inset-0 h-full w-full p-[4%] opacity-40"
                        } else {
                            "pointer-events-none absolute inset-0 h-full w-full p-[4%] drop-shadow-sm"
                        };
                        view! { <img src=piece_asset(p) alt=piece_label(p) draggable="false" class=img_class /> }
                    })
            }}

            // --- quiet-move hint dot (above empty squares) ---
            <Show when=move || {
                let vs = view_state.get();
                vs.dest && !vs.capture
            }>
                <div class="pointer-events-none absolute inset-0 flex items-center justify-center">
                    <div class="h-[28%] w-[28%] rounded-full bg-emerald-950/40"></div>
                </div>
            </Show>
        </div>
    }
}
