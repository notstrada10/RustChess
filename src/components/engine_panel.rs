//! Opponent panel: play a human, play the engine, or watch engine vs engine.

use leptos::prelude::*;

use super::icons;
use crate::engine::Level;
use crate::store::{EngineSide, Store};

#[component]
pub fn EnginePanel() -> impl IntoView {
    let store = expect_context::<Store>();
    let enabled = Memo::new(move |_| store.engine.get().enabled);

    let mode_btn_class = move |computer: bool| {
        let active = enabled.get() == computer;
        if active {
            "flex flex-1 items-center justify-center gap-2 rounded-md bg-emerald-600 px-3 py-1.5 text-sm font-medium text-white shadow"
                .to_string()
        } else {
            "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium text-slate-400 transition-colors hover:text-slate-200"
                .to_string()
        }
    };

    let side_btn = move |side: EngineSide| {
        let btn_class = move || {
            let active = store.engine.get().plays == side;
            if active {
                "flex-1 rounded-md bg-slate-600 px-2 py-1.5 text-sm font-medium text-white shadow"
                    .to_string()
            } else {
                "flex-1 rounded-md px-2 py-1.5 text-sm font-medium text-slate-400 transition-colors hover:text-slate-200"
                    .to_string()
            }
        };
        let title = match side {
            EngineSide::White => "Engine plays the white pieces",
            EngineSide::Black => "Engine plays the black pieces",
            EngineSide::Both => "Engine vs engine: it plays itself while you watch",
        };
        view! {
            <button class=btn_class title=title on:click=move |_| store.set_engine_side(side)>
                {side.label()}
            </button>
        }
    };

    let level_btn = move |level: Level, index: usize| {
        let btn_class = move || {
            let active = store.engine.get().level == level;
            if active {
                "flex-1 rounded-md bg-emerald-600 py-1.5 text-sm font-semibold text-white shadow"
                    .to_string()
            } else {
                "flex-1 rounded-md py-1.5 text-sm font-medium text-slate-400 transition-colors hover:text-slate-200"
                    .to_string()
            }
        };
        view! {
            <button class=btn_class title=level.label() on:click=move |_| store.set_engine_level(level)>
                {index + 1}
            </button>
        }
    };

    view! {
        <div class="flex flex-col gap-3 rounded-xl border border-slate-800 bg-slate-900/70 p-4 shadow-lg shadow-slate-950/40">
            <div class="flex items-center gap-2">
                {icons::cpu()}
                <h2 class="text-xs font-semibold uppercase tracking-widest text-slate-400">
                    "Opponent"
                </h2>
            </div>

            <div class="flex rounded-lg border border-slate-700 bg-slate-950/60 p-0.5">
                <button class=move || mode_btn_class(false) on:click=move |_| store.set_engine_enabled(false)>
                    {icons::users()}
                    "Two players"
                </button>
                <button class=move || mode_btn_class(true) on:click=move |_| store.set_engine_enabled(true)>
                    {icons::cpu()}
                    "Computer"
                </button>
            </div>

            <Show when=move || enabled.get()>
                <div class="flex flex-col gap-3">
                    <div class="flex items-center gap-3">
                        <span class="w-24 shrink-0 text-xs text-slate-500">"Engine plays"</span>
                        <div class="flex flex-1 rounded-lg border border-slate-700 bg-slate-950/60 p-0.5">
                            {EngineSide::ALL.into_iter().map(side_btn).collect_view()}
                        </div>
                    </div>
                    <div class="flex items-center gap-3">
                        <span class="w-24 shrink-0 text-xs text-slate-500">"Strength"</span>
                        <div class="flex flex-1 rounded-lg border border-slate-700 bg-slate-950/60 p-0.5">
                            {Level::ALL
                                .into_iter()
                                .enumerate()
                                .map(|(i, level)| level_btn(level, i))
                                .collect_view()}
                        </div>
                    </div>
                    <p class="text-right text-[11px] text-slate-500">
                        {move || {
                            let cfg = store.engine.get();
                            if cfg.plays == EngineSide::Both {
                                format!("{} — engine plays itself, sit back", cfg.level.label())
                            } else {
                                format!("{} — alpha-beta search, written in Rust", cfg.level.label())
                            }
                        }}
                    </p>
                </div>
            </Show>
        </div>
    }
}
