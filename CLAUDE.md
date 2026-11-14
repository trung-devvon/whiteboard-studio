# Whiteboard Studio — guide for AI assistants

- **Talk to the user in Vietnamese.** Code identifiers in English; code comments and internal docs (`docs/`) may be Vietnamese. `README.md` and this file stay in English.
- The master plan lives in `not_push_git/` (files 00–09, git-ignored). Accepted decisions: `docs/adr/`. Progress: `not_push_git/06-lo-trinh-va-tieu-chi-hoan-thanh.md`.
- Current phase: **Phase 1 done on branch `phase-1-core-engine`** (`docs/giai-doan-1/`) → next is Phase 2 (handwritten text with `harfrust` + `skrifa`, flat-color images, SVG icons). Open decision: TTS provider (ADR-0012).
- Skeletonization uses **Guo–Hall**, not Zhang–Suen (ADR-0020). Run `UPDATE_GOLDEN=1 cargo test -p wb-cli` only when a visual change is intended, and review the PNGs in `tests/golden/`.

## Architecture rules
- Logic lives in `crates/wb-*`; `src-tauri/` is a thin IPC shell only.
- `wb-engine`, `wb-model`, `wb-text`, `wb-planner`, `wb-timing`, `wb-dsl` **must build for `wasm32-unknown-unknown`** — no file I/O, network, FFmpeg or ONNX.
- TypeScript types in `src/bindings/` are **generated** (`npm run gen:bindings`) — never edit by hand.
- Send draw plans to the frontend as binary (postcard + `tauri::ipc::Response`), not JSON.
- Rendering follows the reveal principle (ADR-0001) and must be deterministic (seeded RNG).
- `spikes/`, `src/spike/` and `src-tauri/src/spike.rs` are temporary Phase 0 prototypes (marked `TẠM`); remove them at the start of Phase 3.

## Licensing policy ("keep doors open")
- Only permissive licenses for crates, npm packages and assets; run `cargo deny check` after adding a dependency.
- No embedded GPL/AGPL code; FFmpeg is used only as a separate LGPL process.
- Never integrate services that require reverse-engineered tokens or DRM circumvention (e.g. Edge-TTS — ADR-0012).
- Record every third-party asset in `assets/CREDITS.md`.

## Before reporting work as done
```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx tsc --noEmit
```
