# AGENTS.md — handoff guide for any AI coding agent

Read this file first, then `CLAUDE.md` (hard rules) and `docs/BAN-GIAO.md` (Vietnamese handoff: status, roadmap, open items).
You should not need to ask the owner to re-explain the project.

## 1. What this is
**Whiteboard Studio**: a Windows desktop app (Rust + Tauri v2 + React) that turns images, handwritten text and icons
into "hand drawing on a whiteboard" videos, with narration (Azure TTS) synced to the drawing, pen sound and subtitles.
Everything runs locally except text-to-speech. Owner: a Vietnamese student; **reply in Vietnamese**.

## 2. Pipeline (how a project becomes a video)
```text
project (.wbproj folder: project.json + assets/ + generated/tts/)
  └─ wb-compile::prepare(project, ctx, cache)
       ├─ per element: analyze (wb-vision image → strokes | wb-text glyph skeletons | SVG paths) → layers + DrawPlan
       ├─ per scene: SceneNarration::load → cached Azure speech (generated/tts/<key>.wav|json) → [n] bookmarks
       └─ Bundle (postcard): ink layers, placement, plans, timing, anchors, hand sprite
  ├─ Bundle::timeline() → wb-timing::resolve: stroke slots; elements anchored to narration marks
  ├─ Prepared::soundtrack(): narration + procedural pen sound (wb-audio::sfx) + subtitle cues
  ├─ preview: Tauri `compile_preview` → [json][bundle][wav] → WASM Player (wb-player) on a canvas + Web Audio
  └─ export: Engine (tiny-skia reveal) → raw RGBA → FFmpeg (separate LGPL process) + WAV + SRT → MP4
```

## 3. Repository map
| Path | Role |
|---|---|
| `crates/wb-model` | Project document types (serde, specta → `src/bindings/model.ts`). WASM-safe. |
| `crates/wb-vision` | Image analysis: threshold, Guo–Hall skeleton, stroke graph, background removal. |
| `crates/wb-text` | Handwriting: font shaping (harfrust/skrifa), glyph skeletons, Vietnamese diacritic order. WASM-safe. |
| `crates/wb-planner` | Stroke ordering/smoothing, hatch fills. WASM-safe. |
| `crates/wb-timing` | Timeline: stroke slots, minimum-jerk pen motion, narration anchors + squeeze. WASM-safe. |
| `crates/wb-engine` | Renderer (reveal masks + hand sprite), `Bundle` format. WASM-safe. |
| `crates/wb-player` | wasm-bindgen Player for the in-app preview (`npm run build:wasm`). |
| `crates/wb-compile` | Project → Bundle (shared by CLI and app), analysis cache, icons index, hands library, narration/soundtrack. |
| `crates/wb-project` | `.wbproj` folder I/O (atomic save, autosave, recovery, migration), sample projects (`SAMPLES`). |
| `crates/wb-audio` | Azure TTS client (official WebSocket + user key), SSML, keyring credentials, WAV, mixer, pen SFX, subtitles. Native only. |
| `crates/wb-export` | FFmpeg encoder pipeline (nvenc → amf → qsv → openh264), audio + subtitle inputs. |
| `crates/wb-cli` | `wb` CLI: render, frame, quick, analyze, bundle, remove-bg, tts. |
| `src-tauri/` | Thin IPC shell (`commands.rs`). No logic here. |
| `src/` | React 19 UI: `App.tsx` (welcome + editor), `components/`, `store.ts` (zustand + zundo undo), `player.ts`, `audio.ts`, `api.ts`, `theme.ts`. |
| `src/dev/mockIpc.ts` | Mocks Tauri commands so the UI runs in a plain browser (`npm run dev`). |
| `assets/` | Built-in hands, fonts (Patrick Hand), Lucide icons, sample SVGs. Referenced as `builtin:…`. Credits in `assets/CREDITS.md`. |
| `docs/` | `TIEN-TRINH.md` (short progress), `BAN-GIAO.md` (handoff), `adr/` (decisions), `giai-doan-N/` (per-phase notes). |
| `.claude/skills/whiteboard-ui/` | UI style rules ("Giấy & Mực", 5 colours) + `check_palette.py`. Follow for any UI change. |
| `not_push_git/` | Owner's private master plan (git-ignored, may be absent on other machines). `docs/BAN-GIAO.md` summarises it. |

## 4. Commands
```bash
npm install                      # once
npm run build:wasm               # after changing wb-engine/wb-timing/wb-model/wb-player (Bundle format!)
npm run tauri dev                # run the desktop app (needs port 1420 free)
npm run dev                      # UI only in a browser with mocked IPC (needs target/preview-bundle.bin)
cargo run -p wb-cli -- bundle tests/fixtures/projects/demo-phase2.json -o target/preview-bundle.bin
npm run gen:bindings             # after changing wb-model types (never edit src/bindings by hand)
cargo run -p wb-cli -- tts <project.json>           # create narration audio (uses Azure quota)
cargo run --release -p wb-cli -- render <project.json> -o out.mp4
```
Verify before reporting done:
```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx tsc --noEmit
python .claude/skills/whiteboard-ui/scripts/check_palette.py src   # after UI changes
cargo deny check                                                   # after adding a dependency
```

## 5. Rules that bite
- Logic in `crates/`, never in `src-tauri/`. WASM-safe crates: no file I/O, network, FFmpeg, ONNX.
- Changing `Bundle` (wb-engine) breaks old `.bin` files: rebuild WASM and `target/preview-bundle.bin`.
- Rendering must be deterministic (seeded RNG). Golden images: only `UPDATE_GOLDEN=1` when a visual change is intended.
- Permissive licences only; FFmpeg as a separate process; no Edge-TTS or any reverse-engineered/DRM access (ADR-0012).
- Azure key: OS keyring or git-ignored `.env` (`WB_AZURE_SPEECH_KEY`, `WB_AZURE_SPEECH_REGION=japaneast`). Never print, log or commit it.
- `cargo test -p wb-audio -- --ignored` calls Azure for real (spends quota) — only when needed.
- Patrick Hand has no glyphs for symbols like `→`, `·`; use plain ASCII in sample text.
- Stop any dev/preview server you started on port 1420 before ending your turn.
- Commit/push only when the owner asks; the owner usually runs git commands themselves (see `docs/BAN-GIAO.md` §6).

## 6. Where to continue
See `docs/BAN-GIAO.md` §4 (next steps, in priority order). Keep `docs/TIEN-TRINH.md` (short, Vietnamese) and the
relevant `docs/giai-doan-N/README.md` updated whenever a step is finished.
