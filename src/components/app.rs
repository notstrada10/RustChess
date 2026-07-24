//! Application shell: layout, header, global keyboard shortcuts.

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::board::ChessBoard;
use super::engine_panel::EnginePanel;
use super::fen_bar::FenBar;
use super::promotion::PromotionModal;
use super::setup_tray::SetupTray;
use super::sidebar::{MoveList, NavControls, StatusBanner};
use crate::state::AppMode;
use crate::store::Store;

/// `true` when the event originates from a text-entry element, in which case
/// global shortcuts must not fire.
fn typing_in_input(ev: &web_sys::KeyboardEvent) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|el| {
            let tag = el.tag_name();
            tag == "INPUT" || tag == "TEXTAREA" || el.is_content_editable()
        })
        .unwrap_or(false)
}

#[component]
pub fn App() -> impl IntoView {
    let store = Store::new();
    provide_context(store);

    // Whenever the latest position belongs to the engine, let it think.
    // Aborted/finished searches flip `thinking` back, re-running this effect.
    Effect::new(move |_| {
        if store.engine_should_move() {
            store.spawn_engine_search();
        }
    });

    // Live suggestions: analyse the displayed position whenever the toggle is
    // on and it's a human's move; `analyzed_gen` stops re-analysing an
    // unchanged position after the search completes.
    Effect::new(move |_| {
        let wanted = store.suggestions_wanted();
        let analyzing = store.analyzing.get();
        let gen = store.search_gen.get();
        if wanted {
            if !analyzing && store.analyzed_gen.get() != Some(gen) {
                store.spawn_analysis();
            }
        } else if store.suggestion.get_untracked().is_some() {
            store.suggestion.set(None);
        }
    });

    // Global keyboard shortcuts: ← → Home End (timeline), Esc, F (flip).
    let handle = window_event_listener(ev::keydown, move |ev| {
        if typing_in_input(&ev) {
            return;
        }
        let in_play = store.mode.get_untracked() == AppMode::Play;
        match ev.key().as_str() {
            "ArrowLeft" if in_play => {
                ev.prevent_default();
                store.nav_prev();
            }
            "ArrowRight" if in_play => {
                ev.prevent_default();
                store.nav_next();
            }
            "Home" if in_play => {
                ev.prevent_default();
                store.nav_first();
            }
            "End" if in_play => {
                ev.prevent_default();
                store.nav_last();
            }
            "Escape" => store.escape(),
            "f" | "F" => store.flip(),
            "h" | "H" => store.toggle_suggestions(),
            _ => {}
        }
    });
    on_cleanup(move || handle.remove());

    view! {
        <div class="flex min-h-screen flex-col">
            // Ambient glow backdrop behind everything.
            <div class="pointer-events-none fixed inset-0 -z-10 overflow-hidden" aria-hidden="true">
                <div class="absolute -left-40 -top-40 h-[30rem] w-[30rem] rounded-full bg-emerald-500/10 blur-3xl"></div>
                <div class="absolute -bottom-32 -right-32 h-[26rem] w-[26rem] rounded-full bg-sky-500/10 blur-3xl"></div>
                <div class="absolute left-1/2 top-1/3 h-[20rem] w-[36rem] -translate-x-1/2 rounded-full bg-emerald-500/5 blur-3xl"></div>
            </div>
            <Header />
            <main class="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-6 px-4 py-6 sm:px-6 lg:flex-row lg:items-start lg:justify-center">
                <section class="mx-auto flex w-full max-w-[600px] flex-col gap-3 lg:mx-0 lg:max-w-[min(620px,calc(100vh_-_230px))]">
                    <ChessBoard />
                    <FenBar />
                </section>
                <aside class="mx-auto flex w-full max-w-[600px] flex-col gap-3 lg:mx-0 lg:w-[340px] lg:shrink-0">
                    <StatusBanner />
                    {move || match store.mode.get() {
                        AppMode::Play => view! {
                            <EnginePanel />
                            <NavControls />
                            <MoveList />
                        }
                            .into_any(),
                        AppMode::Setup => view! { <SetupTray /> }.into_any(),
                    }}
                </aside>
            </main>
            <footer class="border-t border-slate-900 py-4 text-center text-[11px] text-slate-600">
                "RustChess — 100% Rust · Leptos · Shakmaty · WebAssembly. Piece art by Colin M.L. Burnett (CC BY-SA 3.0). Keys: ← → history, F flip, H engine suggestions, Esc cancel."
            </footer>
            <PromotionModal />
        </div>
    }
}

#[component]
fn Header() -> impl IntoView {
    let store = expect_context::<Store>();

    let seg_class = move |mode: AppMode| {
        let active = store.mode.get() == mode;
        if active {
            "rounded-md bg-emerald-600 px-3.5 py-1.5 text-sm font-medium text-white shadow"
                .to_string()
        } else {
            "rounded-md px-3.5 py-1.5 text-sm font-medium text-slate-400 transition-colors hover:text-slate-100"
                .to_string()
        }
    };

    view! {
        <header class="sticky top-0 z-40 border-b border-slate-800 bg-slate-950/80 backdrop-blur">
            <div class="mx-auto flex h-14 w-full max-w-6xl items-center gap-4 px-4 sm:px-6">
                <div class="flex items-center gap-2.5">
                    <img
                        src="assets/pieces/wN.svg"
                        alt="RustChess"
                        class="h-8 w-8 drop-shadow-[0_0_10px_rgba(52,211,153,0.35)]"
                    />
                    <h1 class="text-lg font-bold tracking-tight">
                        "Rust"
                        <span class="bg-gradient-to-r from-emerald-400 to-sky-400 bg-clip-text text-transparent">
                            "Chess"
                        </span>
                    </h1>
                    <span class="hidden rounded-full border border-slate-700 px-2 py-0.5 text-[10px] uppercase tracking-widest text-slate-500 sm:inline-block">
                        "Rust · WASM"
                    </span>
                </div>
                <div class="ml-auto flex items-center gap-2">
                    <div class="flex rounded-lg border border-slate-700 bg-slate-900 p-0.5">
                        <button
                            class=move || seg_class(AppMode::Play)
                            on:click=move |_| store.set_mode(AppMode::Play)
                        >
                            "Play"
                        </button>
                        <button
                            class=move || seg_class(AppMode::Setup)
                            on:click=move |_| store.set_mode(AppMode::Setup)
                        >
                            "Setup"
                        </button>
                    </div>
                </div>
            </div>
        </header>
    }
}
