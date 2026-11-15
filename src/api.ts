// Lớp gọi lệnh Tauri (vỏ mỏng ở src-tauri/src/commands.rs). Kiểu dữ liệu dự án: src/bindings (sinh tự động).
import { Channel, invoke } from "@tauri-apps/api/core";
import type { Project } from "./bindings/model";

export type Opened = { root: string; project: Project; recovery: Project | null };
export type HandInfo = { path: string; name: string };

export type ElementInfo = {
  scene: string;
  id: string;
  kind: string;
  style: string;
  layers: string[];
  strokes: number;
  coverage: number;
  analyze_ms: number;
};
export type PreviewHeader = { elements: ElementInfo[]; compile_ms: number };

export type ExportEvent =
  | { type: "preparing" }
  | { type: "progress"; frame: number; total: number }
  | { type: "done"; path: string; seconds: number; encoder: string };

export const api = {
  create: (path: string) => invoke<Opened>("project_create", { path }),
  open: (path: string) => invoke<Opened>("project_open", { path }),
  save: (root: string, project: Project) => invoke<void>("project_save", { root, project }),
  autosave: (root: string, project: Project) => invoke<void>("project_autosave", { root, project }),
  discardAutosave: (root: string) => invoke<void>("project_discard_autosave", { root }),
  importAsset: (root: string, src: string) => invoke<string>("asset_import", { root, src }),
  builtinHands: () => invoke<HandInfo[]>("builtin_hands"),

  /** Biên dịch cho Player: trả báo cáo + gói nhị phân (Bundle, postcard). */
  async compilePreview(root: string, project: Project, width: number) {
    const buf = await invoke<ArrayBuffer>("compile_preview", { root, project, width });
    const view = new DataView(buf);
    const n = view.getUint32(0, true);
    const header = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 4, n))) as PreviewHeader;
    return { header, bundle: new Uint8Array(buf, 4 + n) };
  },

  exportVideo(root: string, project: Project, out: string, width: number | null, onEvent: (e: ExportEvent) => void) {
    const ch = new Channel<ExportEvent>();
    ch.onmessage = onEvent;
    return invoke<void>("export_video", { root, project, out, width, onEvent: ch });
  },
  exportCancel: () => invoke<void>("export_cancel"),
};

/** Lỗi từ lệnh Tauri là chuỗi; lỗi JS là Error. */
export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
