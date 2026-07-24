//! UI components and small shared view helpers.

mod app;
mod board;
mod engine_panel;
mod fen_bar;
mod promotion;
mod setup_tray;
mod sidebar;

pub use app::App;

use shakmaty::{Color, Piece, Role};

/// Path (served by trunk's `copy-dir`) of the SVG sprite for a piece.
pub fn piece_asset(piece: Piece) -> String {
    format!(
        "assets/pieces/{}{}.svg",
        if piece.color.is_white() { 'w' } else { 'b' },
        piece.role.upper_char()
    )
}

pub fn color_name(color: Color) -> &'static str {
    color.fold_wb("White", "Black")
}

pub fn role_name(role: Role) -> &'static str {
    match role {
        Role::Pawn => "pawn",
        Role::Knight => "knight",
        Role::Bishop => "bishop",
        Role::Rook => "rook",
        Role::Queen => "queen",
        Role::King => "king",
    }
}

/// Accessible label like "white knight".
pub fn piece_label(piece: Piece) -> String {
    format!(
        "{} {}",
        if piece.color.is_white() { "white" } else { "black" },
        role_name(piece.role)
    )
}

/// Tiny inline SVG icon set (stroke = currentColor) so the app ships with
/// zero icon dependencies.
pub mod icons {
    use leptos::prelude::*;

    macro_rules! icon {
        ($name:ident, $view_box:literal, $body:literal) => {
            pub fn $name() -> impl IntoView {
                view! {
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox=$view_box
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        class="h-4 w-4 shrink-0"
                        aria-hidden="true"
                        inner_html=$body
                    ></svg>
                }
            }
        };
    }

    icon!(chevron_left, "0 0 24 24", r#"<path d="M15 18l-6-6 6-6"/>"#);
    icon!(chevron_right, "0 0 24 24", r#"<path d="M9 6l6 6-6 6"/>"#);
    icon!(
        chevrons_left,
        "0 0 24 24",
        r#"<path d="M11 17l-5-5 5-5"/><path d="M18 17l-5-5 5-5"/>"#
    );
    icon!(
        chevrons_right,
        "0 0 24 24",
        r#"<path d="M13 7l5 5-5 5"/><path d="M6 7l5 5-5 5"/>"#
    );
    icon!(
        flip,
        "0 0 24 24",
        r#"<path d="M7 16V4"/><path d="M3 8l4-4 4 4"/><path d="M17 8v12"/><path d="M21 16l-4 4-4-4"/>"#
    );
    icon!(plus, "0 0 24 24", r#"<path d="M12 5v14"/><path d="M5 12h14"/>"#);
    icon!(
        copy,
        "0 0 24 24",
        r#"<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#
    );
    icon!(check, "0 0 24 24", r#"<path d="M20 6L9 17l-5-5"/>"#);
    icon!(
        warning,
        "0 0 24 24",
        r#"<path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#
    );
    icon!(
        trash,
        "0 0 24 24",
        r#"<path d="M3 6h18"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"#
    );
    icon!(
        pointer,
        "0 0 24 24",
        r#"<path d="M4 3l7.07 17 2.51-7.39L21 10.07z"/>"#
    );
    icon!(
        restart,
        "0 0 24 24",
        r#"<path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v5h5"/>"#
    );
    icon!(play, "0 0 24 24", r#"<path d="M6 4l14 8-14 8z"/>"#);
    icon!(
        list,
        "0 0 24 24",
        r#"<path d="M8 6h13"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M3 6h.01"/><path d="M3 12h.01"/><path d="M3 18h.01"/>"#
    );
    icon!(
        wrench,
        "0 0 24 24",
        r#"<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>"#
    );
    icon!(
        flag,
        "0 0 24 24",
        r#"<path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z"/><path d="M4 22v-7"/>"#
    );
    icon!(
        handshake,
        "0 0 24 24",
        r#"<path d="M12 5.5L9.5 3H3v11h3l5 5 6-6"/><path d="M12 5.5L14.5 3H21v11h-3"/><path d="M8 13l3 3"/>"#
    );
    icon!(
        code,
        "0 0 24 24",
        r#"<path d="M16 18l6-6-6-6"/><path d="M8 6l-6 6 6 6"/>"#
    );
    icon!(
        cpu,
        "0 0 24 24",
        r#"<rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 1v3"/><path d="M15 1v3"/><path d="M9 20v3"/><path d="M15 20v3"/><path d="M20 9h3"/><path d="M20 15h3"/><path d="M1 9h3"/><path d="M1 15h3"/>"#
    );
    icon!(
        users,
        "0 0 24 24",
        r#"<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>"#
    );
    icon!(
        bulb,
        "0 0 24 24",
        r#"<path d="M9 18h6"/><path d="M10 22h4"/><path d="M12 2a7 7 0 0 0-4.5 12.4c.9.7 1.5 1.6 1.5 2.6h6c0-1 .6-1.9 1.5-2.6A7 7 0 0 0 12 2z"/>"#
    );
}
