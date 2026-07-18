//! Right-hand panel: status banner, timeline controls and the SAN move list.

use leptos::prelude::*;
use shakmaty::Color;

use super::{color_name, icons};
use crate::state::{game_status, move_rows, AppMode, GameStatus, MoveRow};
use crate::store::Store;

#[component]
pub fn StatusBanner() -> impl IntoView {
    let store = expect_context::<Store>();
    let status = Memo::new(move |_| store.game.with(|g| game_status(&g.displayed().pos)));
    let in_setup = Memo::new(move |_| store.mode.get() == AppMode::Setup);
    let progress = Memo::new(move |_| store.game.with(|g| (g.cursor(), g.history().len() - 1)));

    let banner_class = move || {
        let base = "flex items-center gap-3 rounded-xl border px-4 py-3 text-sm font-medium ";
        if in_setup.get() {
            return format!("{base}border-slate-700 bg-slate-900/70 text-slate-300");
        }
        match status.get() {
            GameStatus::Turn(_) => format!("{base}border-slate-700 bg-slate-900/70 text-slate-200"),
            GameStatus::Check(_) => format!("{base}border-amber-500/50 bg-amber-500/10 text-amber-300"),
            GameStatus::Checkmate(_) => format!("{base}border-rose-500/50 bg-rose-500/10 text-rose-300"),
            GameStatus::Stalemate | GameStatus::InsufficientMaterial => {
                format!("{base}border-sky-500/50 bg-sky-500/10 text-sky-300")
            }
        }
    };

    let turn_dot = move |color: Color| {
        let class = if color == Color::White {
            "inline-block h-3 w-3 rounded-full bg-slate-100 ring-1 ring-slate-400"
        } else {
            "inline-block h-3 w-3 rounded-full bg-slate-950 ring-1 ring-slate-500"
        };
        view! { <span class=class></span> }
    };

    let content = move || {
        if in_setup.get() {
            return view! {
                <span class="flex items-center gap-2">
                    {icons::wrench()}
                    <span>"Setup mode — build any position, then press play"</span>
                </span>
            }
            .into_any();
        }
        match status.get() {
            GameStatus::Turn(c) => view! {
                <span class="flex items-center gap-2.5">
                    {turn_dot(c)}
                    <span>{color_name(c)} " to move"</span>
                </span>
            }
            .into_any(),
            GameStatus::Check(c) => view! {
                <span class="flex items-center gap-2.5">
                    {icons::warning()}
                    <span>{color_name(c)} " to move — check!"</span>
                </span>
            }
            .into_any(),
            GameStatus::Checkmate(winner) => view! {
                <span class="flex items-center gap-2.5">
                    {icons::flag()}
                    <span class="font-semibold">"Checkmate — " {color_name(winner)} " wins"</span>
                </span>
            }
            .into_any(),
            GameStatus::Stalemate => view! {
                <span class="flex items-center gap-2.5">
                    {icons::handshake()}
                    <span class="font-semibold">"Stalemate — draw"</span>
                </span>
            }
            .into_any(),
            GameStatus::InsufficientMaterial => view! {
                <span class="flex items-center gap-2.5">
                    {icons::handshake()}
                    <span class="font-semibold">"Draw — insufficient material"</span>
                </span>
            }
            .into_any(),
        }
    };

    view! {
        <div class=banner_class>
            {content}
            <Show when=move || { !in_setup.get() && progress.get().1 > 0 }>
                <span class="ml-auto text-xs font-normal text-slate-500">
                    {move || {
                        let (cursor, total) = progress.get();
                        format!("move {cursor}/{total}")
                    }}
                </span>
            </Show>
        </div>
    }
}

#[component]
pub fn NavControls() -> impl IntoView {
    let store = expect_context::<Store>();
    let at_start = Memo::new(move |_| store.game.with(|g| g.at_start()));
    let at_latest = Memo::new(move |_| store.game.with(|g| g.at_latest()));

    let nav_btn = "flex h-9 flex-1 items-center justify-center rounded-lg border border-slate-700/60 bg-slate-800 text-slate-200 transition-colors hover:bg-slate-700 disabled:pointer-events-none disabled:opacity-30";
    let aux_btn = "flex h-9 w-9 items-center justify-center rounded-lg border border-slate-700/60 bg-slate-800 text-slate-300 transition-colors hover:bg-slate-700 hover:text-emerald-300";

    view! {
        <div class="flex items-center gap-1.5">
            <button
                class=nav_btn
                title="First position (Home)"
                aria-label="Go to first position"
                disabled=move || at_start.get()
                on:click=move |_| store.nav_first()
            >
                {icons::chevrons_left()}
            </button>
            <button
                class=nav_btn
                title="Previous move (←)"
                aria-label="Go to previous move"
                disabled=move || at_start.get()
                on:click=move |_| store.nav_prev()
            >
                {icons::chevron_left()}
            </button>
            <button
                class=nav_btn
                title="Next move (→)"
                aria-label="Go to next move"
                disabled=move || at_latest.get()
                on:click=move |_| store.nav_next()
            >
                {icons::chevron_right()}
            </button>
            <button
                class=nav_btn
                title="Latest move (End)"
                aria-label="Go to latest move"
                disabled=move || at_latest.get()
                on:click=move |_| store.nav_last()
            >
                {icons::chevrons_right()}
            </button>
            <div class="mx-1 h-6 w-px bg-slate-700/70"></div>
            <button class=aux_btn title="Flip board (F)" aria-label="Flip board" on:click=move |_| store.flip()>
                {icons::flip()}
            </button>
            <button class=aux_btn title="New game" aria-label="New game" on:click=move |_| store.new_game()>
                {icons::plus()}
            </button>
        </div>
    }
}

#[component]
pub fn MoveList() -> impl IntoView {
    let store = expect_context::<Store>();
    let rows = Memo::new(move |_| store.game.with(|g| move_rows(g.history())));
    let cursor = Memo::new(move |_| store.game.with(|g| g.cursor()));

    // Keep the active ply visible as the user navigates.
    Effect::new(move |_| {
        let ply = cursor.get();
        if let Some(el) = document().get_element_by_id(&format!("ply-{ply}")) {
            let opts = web_sys::ScrollIntoViewOptions::new();
            opts.set_block(web_sys::ScrollLogicalPosition::Nearest);
            opts.set_behavior(web_sys::ScrollBehavior::Smooth);
            el.scroll_into_view_with_scroll_into_view_options(&opts);
        }
    });

    let row_key = |row: &MoveRow| {
        format!(
            "{}:{}:{}:{}:{}",
            row.number,
            row.white.as_ref().map_or(0, |w| w.0),
            row.white.as_ref().map_or("", |w| w.1.as_str()),
            row.black.as_ref().map_or(0, |b| b.0),
            row.black.as_ref().map_or("", |b| b.1.as_str()),
        )
    };

    let san_cell = move |entry: Option<(usize, String)>| match entry {
        Some((ply, san)) => {
            let cell_class = move || {
                if cursor.get() == ply {
                    "w-full rounded-md bg-emerald-500/20 px-2 py-1 text-left font-semibold text-emerald-300"
                        .to_string()
                } else {
                    "w-full rounded-md px-2 py-1 text-left text-slate-300 transition-colors hover:bg-slate-800 hover:text-slate-100"
                        .to_string()
                }
            };
            view! {
                <button id=format!("ply-{ply}") class=cell_class on:click=move |_| store.goto(ply)>
                    {san}
                </button>
            }
            .into_any()
        }
        None => view! { <span class="px-2 py-1 text-slate-600">"…"</span> }.into_any(),
    };

    view! {
        <div class="flex min-h-[140px] flex-col overflow-hidden rounded-xl border border-slate-800 bg-slate-900/70">
            <div class="flex items-center gap-2 border-b border-slate-800 px-4 py-2.5">
                {icons::list()}
                <h2 class="text-xs font-semibold uppercase tracking-widest text-slate-400">"Moves"</h2>
            </div>
            <div class="scroll-slim max-h-[300px] flex-1 overflow-y-auto p-2 font-mono text-sm lg:max-h-[calc(100vh_-_430px)]">
                <Show
                    when=move || !rows.get().is_empty()
                    fallback=|| {
                        view! {
                            <p class="px-2 py-4 text-center font-sans text-xs italic text-slate-500">
                                "No moves yet — drop a pawn on e4!"
                            </p>
                        }
                    }
                >
                    <For each=move || rows.get() key=row_key let:row>
                        <div class="grid grid-cols-[2.5rem_1fr_1fr] items-center gap-1">
                            <span class="pr-1 text-right text-xs text-slate-500">
                                {row.number} "."
                            </span>
                            {san_cell(row.white.clone())}
                            {san_cell(row.black.clone())}
                        </div>
                    </For>
                </Show>
            </div>
        </div>
    }
}
