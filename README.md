# Whiteboard Studio

**A desktop studio for creating whiteboard explainer videos — a hand draws your images and text on a board, perfectly in sync with the narration.**

Built with Rust, Tauri v2 and React. Offline-first: image analysis, stroke planning, rendering and encoding all run locally.

> **Status: early development (Phase 0 — foundations & technical validation).** Not usable yet. See the [Phase 0 report](docs/giai-doan-0/ket-qua-spike.md).

---

## Highlights (planned)

- **Draws anything** — line art, flat illustrations, logos, SVG icons and (later) photos, through dedicated image pipelines.
- **Pixel-perfect results** — a *reveal* renderer: the hand's path uncovers the original image, so the finished frame always matches the source.
- **Natural handwriting** — glyph skeletonization with proper Vietnamese diacritic ordering (letters first, tone marks after).
- **Narration-driven timing** — drawing speed adapts so each element finishes exactly when the voice-over says it.
- **Preview = Export** — one Rust engine compiled both natively (export) and to WebAssembly (in-app preview).
- **Fast export** — frames piped straight into FFmpeg with hardware encoding (NVENC, AMF, QSV) and an OpenH264 fallback.
- **Optional AI director** — turn a script and a folder of images into an editable draft project using a compact, low-token DSL. Bring your own key (Gemini, Claude, OpenAI) or run a local model.

## Validated so far (Phase 0)

| Area | Result |
|---|---|
| FFmpeg pipeline (LGPL build) | 1080p30 encoded at ~8× real time (NVENC), color error ≤ 1/255 |
| WebAssembly preview in WebView2 | ~1.5 ms per frame at 960×540 with 2,000 strokes |
| Vietnamese handwriting | 9/9 test lines pass on three OFL fonts, ≥ 99.5% reveal coverage |
| Binary IPC (Rust → UI) | 5 MB draw plan transferred in ~27 ms (4.7× faster than JSON) |

Details and methodology: [`docs/giai-doan-0/ket-qua-spike.md`](docs/giai-doan-0/ket-qua-spike.md).

## Architecture

```text
React + TypeScript UI ──(Tauri IPC: JSON · binary · channels)──► thin Tauri shell
        │                                                              │
        └── WASM build of wb-engine (preview)            Rust core crates (crates/wb-*)
                                                                       │
                                                         FFmpeg sidecar (LGPL) → MP4
```

| Crate | Responsibility |
|---|---|
| `wb-model` | Shared data types; source of the generated TypeScript bindings |
| `wb-vision` | Image pipelines: classification, skeletonization, vectorization |
| `wb-text` | Font loading, shaping, glyph → strokes, diacritic handling |
| `wb-planner` | Stroke ordering, smoothing, fill hatching, user-defined draw regions |
| `wb-timing` | Narration-aware timeline solver and speed control |
| `wb-engine` | Deterministic renderer (no I/O, WASM-compatible) |
| `wb-audio` | TTS providers, alignment, sound effects, mixing |
| `wb-export` | FFmpeg orchestration, chunked parallel rendering, subtitles |
| `wb-dsl` / `wb-ai` | AI director: compact DSL compiler and LLM providers |
| `wb-cli` | Headless command-line renderer |

Architecture decisions are recorded as ADRs in [`docs/adr/`](docs/adr/README.md).

## Repository layout

```text
crates/      Rust core libraries (wb-*)
src-tauri/   Tauri application shell
src/         React frontend (src/bindings/ is generated from Rust — do not edit)
spikes/      Phase 0 prototypes (temporary)
docs/        ADRs and phase reports
tests/       Test fixtures (OFL fonts, sample images, golden frames)
assets/      Bundled resources and CREDITS.md
```

## Getting started

**Prerequisites:** Rust (stable), Node.js 20+, Git, and on Windows the WebView2 runtime (preinstalled on Windows 10/11).

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
cargo install cargo-deny --locked
npm install
npm run build:wasm
npm run tauri dev
```

FFmpeg is not committed. Download a **win64 LGPL** build from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds/releases) and extract it into `tools/ffmpeg/`, or set `WB_FFMPEG` to an `ffmpeg` executable.

### Common commands

| Task | Command |
|---|---|
| Run the app | `npm run tauri dev` |
| Rust tests | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| License & advisory check | `cargo deny check` |
| Regenerate TypeScript types | `npm run gen:bindings` |
| Type-check the frontend | `npm run typecheck` |

## Roadmap

| Phase | Scope |
|---|---|
| 0 | Foundations & technical spikes ✅ |
| 1 | Core engine + CLI: line art → MP4 |
| 2 | Handwritten text, flat-color images, SVG icons |
| 3 | Desktop editor |
| 4 | Narration & sync → **MVP / alpha** |
| 5–7 | Advanced studio tools, local AI for photos, AI director |
| 8 | Polish, signing, auto-update → **v1.0** |

## Licensing

The project license has not been chosen yet. Dependencies are restricted to permissive licenses (enforced by `cargo deny`), and FFmpeg is used only as a separate LGPL process. Third-party assets are listed in [`assets/CREDITS.md`](assets/CREDITS.md).
