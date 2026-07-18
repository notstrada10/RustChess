//! FEN input/output bar: live position FEN, copy to clipboard, load from paste.

use leptos::prelude::*;

use super::icons;
use crate::store::Store;

#[component]
pub fn FenBar() -> impl IntoView {
    let store = expect_context::<Store>();

    // Keep the input in sync with whatever the board shows. The user can
    // freely edit the draft; any board change rewrites it with the live FEN.
    Effect::new(move |_| {
        let fen = store.current_fen();
        store.fen_draft.set(fen);
    });

    let copy_btn_class = move || {
        if store.copied.get() {
            "flex items-center gap-1.5 rounded-lg border border-emerald-500/60 bg-emerald-500/15 px-3 py-2 text-xs font-medium text-emerald-300 transition-colors"
                .to_string()
        } else {
            "flex items-center gap-1.5 rounded-lg border border-slate-700/60 bg-slate-800 px-3 py-2 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700"
                .to_string()
        }
    };

    view! {
        <div class="flex flex-col gap-1.5">
            <label class="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-widest text-slate-500">
                {icons::code()}
                "FEN — paste a position and press Enter or Load"
            </label>
            <div class="flex gap-2">
                <input
                    type="text"
                    spellcheck="false"
                    autocomplete="off"
                    class="min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 font-mono text-[11px] text-slate-200 outline-none transition-colors focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/50 sm:text-xs"
                    prop:value=move || store.fen_draft.get()
                    on:input=move |ev| store.fen_draft.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            store.load_fen();
                        }
                    }
                />
                <button class=copy_btn_class title="Copy FEN to clipboard" on:click=move |_| store.copy_fen()>
                    {move || if store.copied.get() { icons::check().into_any() } else { icons::copy().into_any() }}
                    <span>{move || if store.copied.get() { "Copied" } else { "Copy" }}</span>
                </button>
                <button
                    class="rounded-lg bg-emerald-600 px-3.5 py-2 text-xs font-semibold text-white shadow transition-colors hover:bg-emerald-500"
                    title="Load this FEN onto the board"
                    on:click=move |_| store.load_fen()
                >
                    "Load"
                </button>
            </div>
            {move || {
                store
                    .error
                    .get()
                    .map(|msg| {
                        view! {
                            <div class="flex items-center gap-2 rounded-lg border border-rose-500/40 bg-rose-500/10 px-3 py-2 text-xs text-rose-300">
                                {icons::warning()}
                                <span>{msg}</span>
                                <button
                                    class="ml-auto rounded px-1.5 text-rose-400 transition-colors hover:bg-rose-500/20 hover:text-rose-200"
                                    aria-label="Dismiss error"
                                    on:click=move |_| store.error.set(None)
                                >
                                    "✕"
                                </button>
                            </div>
                        }
                    })
            }}
        </div>
    }
}
