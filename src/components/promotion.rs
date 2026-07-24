//! Modal promotion picker shown when a pawn reaches the last rank.

use leptos::prelude::*;
use shakmaty::Piece;

use super::{piece_asset, piece_label};
use crate::state::{click_moves, promotion_roles};
use crate::store::Store;

#[component]
pub fn PromotionModal() -> impl IntoView {
    let store = expect_context::<Store>();

    view! {
        {move || {
            store
                .promo
                .get()
                .map(|pending| {
                    // Offer exactly the promotion roles that are legal here.
                    let roles = store.game.with_untracked(|g| {
                        promotion_roles(&click_moves(&g.displayed().pos, pending.from, pending.to))
                    });
                    let choices = roles
                        .into_iter()
                        .map(|role| {
                            let piece = Piece {
                                color: pending.color,
                                role,
                            };
                            view! {
                                <button
                                    class="h-16 w-16 rounded-xl bg-slate-800 p-1.5 ring-1 ring-slate-700 transition-all hover:scale-105 hover:bg-emerald-500/20 hover:ring-emerald-400 sm:h-20 sm:w-20"
                                    title=format!("Promote to {}", piece_label(piece))
                                    on:click=move |_| store.choose_promotion(role)
                                >
                                    <img src=piece_asset(piece) alt=piece_label(piece) draggable="false" class="h-full w-full" />
                                </button>
                            }
                        })
                        .collect_view();
                    view! {
                        <div
                            class="overlay-fade fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 backdrop-blur-sm"
                            on:click=move |_| store.cancel_promotion()
                        >
                            <div
                                class="modal-pop flex flex-col items-center gap-4 rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl"
                                on:click=move |ev| ev.stop_propagation()
                            >
                                <p class="text-sm font-medium text-slate-300">"Promote your pawn to:"</p>
                                <div class="flex gap-3">{choices}</div>
                                <button
                                    class="text-xs text-slate-500 transition-colors hover:text-slate-300"
                                    on:click=move |_| store.cancel_promotion()
                                >
                                    "Cancel (Esc)"
                                </button>
                            </div>
                        </div>
                    }
                })
        }}
    }
}
