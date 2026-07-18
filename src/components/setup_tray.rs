//! Setup-mode sandbox tray: piece palette, tools, side to move and actions.

use leptos::prelude::*;
use shakmaty::{Color, Piece, Role};

use super::{color_name, icons, piece_asset, piece_label};
use crate::state::SetupTool;
use crate::store::Store;

const PALETTE_ROLES: [Role; 6] = [
    Role::King,
    Role::Queen,
    Role::Rook,
    Role::Bishop,
    Role::Knight,
    Role::Pawn,
];

#[component]
pub fn SetupTray() -> impl IntoView {
    let store = expect_context::<Store>();
    let validity = Memo::new(move |_| store.setup_validity().err());

    let palette_row = move |color: Color| {
        PALETTE_ROLES
            .into_iter()
            .map(|role| {
                let piece = Piece { color, role };
                let btn_class = move || {
                    let active = store.tool.get() == SetupTool::Place(piece);
                    if active {
                        "aspect-square rounded-lg bg-emerald-500/20 p-1 ring-2 ring-emerald-400"
                            .to_string()
                    } else {
                        "aspect-square rounded-lg bg-slate-800/70 p-1 transition-colors hover:bg-slate-700"
                            .to_string()
                    }
                };
                view! {
                    <button
                        class=btn_class
                        title=format!("Place a {}", piece_label(piece))
                        on:click=move |_| store.toggle_place_tool(piece)
                    >
                        <img src=piece_asset(piece) alt=piece_label(piece) draggable="false" class="h-full w-full" />
                    </button>
                }
            })
            .collect_view()
    };

    let tool_btn_class = move |tool: SetupTool| {
        let active = store.tool.get() == tool;
        if active {
            "flex flex-1 items-center justify-center gap-2 rounded-lg bg-emerald-500/20 px-3 py-2 text-sm font-medium text-emerald-300 ring-2 ring-emerald-400".to_string()
        } else {
            "flex flex-1 items-center justify-center gap-2 rounded-lg border border-slate-700/60 bg-slate-800 px-3 py-2 text-sm font-medium text-slate-300 transition-colors hover:bg-slate-700".to_string()
        }
    };

    let turn_btn_class = move |color: Color| {
        let active = store.setup_turn.get() == color;
        if active {
            "flex-1 rounded-md bg-emerald-600 px-3 py-1.5 text-sm font-medium text-white shadow"
                .to_string()
        } else {
            "flex-1 rounded-md px-3 py-1.5 text-sm font-medium text-slate-400 transition-colors hover:text-slate-200"
                .to_string()
        }
    };

    let action_btn = "flex items-center justify-center gap-2 rounded-lg border border-slate-700/60 bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700";

    view! {
        <div class="flex flex-col gap-4 rounded-xl border border-slate-800 bg-slate-900/70 p-4">
            <div>
                <h2 class="mb-2 text-xs font-semibold uppercase tracking-widest text-slate-400">
                    "Piece palette"
                </h2>
                <div class="grid grid-cols-6 gap-1.5">
                    {palette_row(Color::White)}
                    {palette_row(Color::Black)}
                </div>
            </div>

            <div class="flex gap-2">
                <button
                    class=move || tool_btn_class(SetupTool::Pointer)
                    title="Pointer: pick up and move pieces already on the board"
                    on:click=move |_| store.set_tool(SetupTool::Pointer)
                >
                    {icons::pointer()}
                    "Move"
                </button>
                <button
                    class=move || tool_btn_class(SetupTool::Erase)
                    title="Eraser: remove pieces (right-click always erases too)"
                    on:click=move |_| store.set_tool(SetupTool::Erase)
                >
                    {icons::trash()}
                    "Erase"
                </button>
            </div>

            {move || {
                if let SetupTool::Carry { piece, .. } = store.tool.get() {
                    Some(
                        view! {
                            <p class="rounded-lg border border-sky-500/40 bg-sky-500/10 px-3 py-2 text-xs text-sky-300">
                                "Carrying a " {piece_label(piece)}
                                " — click a square to drop it (Esc to cancel)."
                            </p>
                        },
                    )
                } else {
                    None
                }
            }}

            <div>
                <h2 class="mb-2 text-xs font-semibold uppercase tracking-widest text-slate-400">
                    "Side to move"
                </h2>
                <div class="flex rounded-lg border border-slate-700 bg-slate-950/60 p-0.5">
                    <button class=move || turn_btn_class(Color::White) on:click=move |_| store.setup_turn.set(Color::White)>
                        {color_name(Color::White)}
                    </button>
                    <button class=move || turn_btn_class(Color::Black) on:click=move |_| store.setup_turn.set(Color::Black)>
                        {color_name(Color::Black)}
                    </button>
                </div>
            </div>

            <div class="grid grid-cols-2 gap-2">
                <button class=action_btn title="Remove every piece" on:click=move |_| store.clear_board()>
                    {icons::trash()}
                    "Clear board"
                </button>
                <button class=action_btn title="Reset to the standard starting position" on:click=move |_| store.starting_position()>
                    {icons::restart()}
                    "Start position"
                </button>
            </div>

            <div class="flex flex-col gap-2">
                <button
                    class="flex items-center justify-center gap-2 rounded-lg bg-emerald-600 px-3 py-2.5 text-sm font-semibold text-white shadow transition-colors hover:bg-emerald-500 disabled:pointer-events-none disabled:opacity-40"
                    disabled=move || validity.get().is_some()
                    on:click=move |_| store.play_from_setup()
                >
                    {icons::play()}
                    "Play from this position"
                </button>
                {move || {
                    validity
                        .get()
                        .map(|msg| {
                            view! {
                                <p class="flex items-start gap-2 text-xs text-amber-400">
                                    {icons::warning()}
                                    <span>{msg}</span>
                                </p>
                            }
                        })
                }}
            </div>

            <p class="text-[11px] leading-relaxed text-slate-500">
                "Castling rights are granted automatically for kings and rooks standing on their home squares. Right-click any square to erase it."
            </p>
        </div>
    }
}
