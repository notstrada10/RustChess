# ♞ RustChess

A web-based chess playground written **100% in Rust** — [Leptos](https://leptos.dev)
reactive UI + [Shakmaty](https://github.com/niklasf/shakmaty) rules engine,
compiled to WebAssembly and styled with Tailwind CSS.

![Rust](https://img.shields.io/badge/Rust-WASM-emerald) ![Leptos](https://img.shields.io/badge/Leptos-0.8-blue) ![Shakmaty](https://img.shields.io/badge/Shakmaty-0.30-green)

## Features

- **Play mode** — full rules enforcement via shakmaty: legal-move hints,
  captures, castling (click e1→g1), en passant, promotions (with picker
  modal), check / checkmate / stalemate / insufficient-material detection.
- **Setup mode** — a complete sandbox: a 12-piece palette to stamp pieces onto
  any square, a pointer tool to move pieces, an eraser (right-click also
  erases), side-to-move selector, *Clear board* / *Start position* buttons and
  live position validation before playing on. Castling rights are derived
  automatically from kings/rooks on their home squares.
- **Timeline navigation** — every move is kept in a navigable history:
  `←` / `→` step through moves, `Home` / `End` jump to the ends, and the
  ⏮ ◀ ▶ ⏭ buttons or any move in the SAN move list jump directly. Playing a
  new move while viewing history truncates the future (standard branching).
- **FEN integration** — the live FEN of whatever the board shows is always in
  the input bar; *Copy* puts it on the clipboard, and pasting any FEN +
  `Enter`/`Load` restores it (graceful errors for invalid strings — the app
  never panics).
- **Polish** — last-move & check highlights, board flip (`F`), keyboard-first
  UX, responsive dark "tech" theme, zero-dependency inline SVG icons.

## Prerequisites

```sh
rustup target add wasm32-unknown-unknown
# trunk (WASM bundler): prebuilt binaries at https://github.com/trunk-rs/trunk/releases
cargo install trunk --locked   # or: cargo binstall trunk
```

Trunk automatically downloads matching `wasm-bindgen`, `wasm-opt` and the
standalone Tailwind CLI (pinned in `Trunk.toml`) on first build — no Node
toolchain required.

## Run

```sh
trunk serve            # dev server with hot reload → http://127.0.0.1:8080
trunk build --release  # optimized production bundle in ./dist (~1 MB wasm)
cargo test             # native unit tests for the chess/timeline logic
```

## Architecture

```
src/
├── main.rs              # WASM entry point, mounts <App/>
├── state.rs             # PURE domain logic (no framework): Game timeline,
│                        #   FEN parse/generate, castling detection, SAN rows,
│                        #   game status — fully unit-tested natively
├── store.rs             # Leptos signal store wrapping state.rs + all actions
└── components/
    ├── app.rs           # Shell, header, global keyboard shortcuts
    ├── board.rs         # 8×8 CSS-grid board, per-square reactive memos
    ├── sidebar.rs       # Status banner, nav controls, SAN move list
    ├── setup_tray.rs    # Piece palette, tools, setup actions
    ├── fen_bar.rs       # FEN input / copy / load
    ├── promotion.rs     # Promotion picker modal
    └── mod.rs           # Shared helpers + inline SVG icon set
```

State design: the game timeline (`Vec<Ply>` + cursor) lives in a single
`RwSignal<Game>`; the board renders through per-square `Memo`s so only the
squares that actually changed touch the DOM.

## Credits

Chess piece artwork by Colin M.L. Burnett ("cburnett"), via Wikimedia Commons,
licensed [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/).
