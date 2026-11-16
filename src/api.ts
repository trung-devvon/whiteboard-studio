// Lớp gọi lệnh Tauri (vỏ mỏng ở src-tauri/src/commands.rs). Kiểu dữ liệu dự án: src/bindings (sinh tự động).
import { Channel, invoke } from "@tauri-apps/api/core";
import type { Project } from "./bindings/model";

export type Opened = { root: string; project: Project; recovery: Project | null };
/** Một bộ tay trong thư viện. `path` là giá trị ghi vào `settings.hand`. */
export type HandEntry = { path: string; name: string; editable: boolean; has_wrist: boolean };
/** Nội dung `hand.json` (toạ độ theo px ảnh tay). Trường lạ được giữ nguyên. */
export type HandMeta = {
  name: string;
  file: string;
  anchor: [number, number];
  display_height: number;
  full_scale?: number | null;
  wrist?: [[number, number], [number, number]] | null;
  [extra: string]: unknown;
};
export type IconHit = { name: string; src: string; svg: string };
export type HandDetail ={ meta: HandMeta; width: number; height: number; editable: boolean };

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
  create: (path: string, sample = false) => invoke<Opened>("project_create", { path, sample }),
  open: (path: string) => invoke<Opened>("project_open", { path }),
  save: (root: string, project: Project) => invoke<void>("project_save", { root, project }),
  autosave: (root: string, project: Project) => invoke<void>("project_autosave", { root, project }),
  discardAutosave: (root: string) => invoke<void>("project_discard_autosave", { root }),
  importAsset: (root: string, src: string) => invoke<string>("asset_import", { root, src }),
  /** Tìm icon có sẵn (gõ tiếng Việt hoặc tiếng Anh; rỗng = gợi ý). */
  icons: (query: string, limit?: number) => invoke<IconHit[]>("icons_search", { query, limit }),
  hands: () => invoke<HandEntry[]>("hands_list"),
  handImport: (src: string, name: string) => invoke<string>("hand_import", { src, name }),
  handDuplicate: (path: string) => invoke<string>("hand_duplicate", { path }),
  handLoad: (path: string) => invoke<HandDetail>("hand_load", { path }),
  handSave: (path: string, meta: HandMeta) => invoke<void>("hand_save", { path, meta }),
  handDelete: (path: string) => invoke<void>("hand_delete", { path }),
  /** Ảnh xem trước của bộ tay → URL dùng cho <img> (nhớ `URL.revokeObjectURL`). */
  async handImageUrl(path: string, maxSide: number) {
    const buf = await invoke<ArrayBuffer>("hand_image", { path, maxSide });
    return URL.createObjectURL(new Blob([buf], { type: "image/png" }));
  },

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
