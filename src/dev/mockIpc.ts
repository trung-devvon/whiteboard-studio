// CHỈ DÙNG KHI PHÁT TRIỂN: chạy giao diện trong trình duyệt thường (`npm run dev`, không có Tauri)
// để xem/thử bố cục. Lệnh Rust được giả lập; Player dùng gói biên dịch thật tạo sẵn bằng:
//   wb bundle tests/fixtures/projects/demo-phase2.json -o target/preview-bundle.bin
// Thay đổi trên giao diện không biên dịch lại được (khung Player giữ nguyên).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const DEMO = "/tests/fixtures/projects/demo-phase2.json";
const BUNDLE = "/target/preview-bundle.bin";

export async function installMock() {
  const project = await (await fetch(DEMO)).json();
  const bundle = new Uint8Array(await (await fetch(BUNDLE)).arrayBuffer());
  const header = new TextEncoder().encode(JSON.stringify({ elements: [], compile_ms: 0 }));
  const packed = new Uint8Array(4 + header.length + bundle.length);
  new DataView(packed.buffer).setUint32(0, header.length, true);
  packed.set(header, 4);
  packed.set(bundle, 4 + header.length);

  // thư viện tay giả: đọc thẳng hand.json + ảnh trong assets/hands của repo
  const hands: Record<string, { dir: string; editable: boolean }> = {
    "builtin:hands/pen-brown": { dir: "/assets/hands/pen-brown", editable: false },
    "builtin:hands/pencil-green": { dir: "/assets/hands/pencil-green", editable: false },
    "user:hands/thu": { dir: "/assets/hands/pencil-purple", editable: true },
  };
  const metas: Record<string, unknown> = {};
  for (const [p, h] of Object.entries(hands)) metas[p] = await (await fetch(`${h.dir}/hand.json`)).json();
  const size = async (url: string) => {
    const im = new Image();
    im.src = url;
    await im.decode();
    return { width: im.naturalWidth, height: im.naturalHeight };
  };

  mockIPC(async (cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "project_open":
      case "project_create":
        return { root: "C:/demo/demo-phase2.wbproj", project, recovery: null };
      case "compile_preview":
        return packed.buffer;
      case "icons_search": {
        // giả lập: lọc theo tên trong một danh sách nhỏ
        const names = ["arrow-right", "arrow-left", "arrow-up", "arrow-down", "redo", "lightbulb", "house", "user", "users", "star", "heart", "rocket", "clock", "book-open"];
        const q = String(a.query ?? "").toLowerCase();
        const pick = names.filter((n) => !q || n.includes(q) || (q.includes("tên") && n.startsWith("arrow")));
        return Promise.all(
          pick.map(async (name) => ({
            name,
            src: `builtin:icons/lucide/icons/${name}.svg`,
            svg: await (await fetch(`/assets/icons/lucide/icons/${name}.svg`)).text(),
          })),
        );
      }
      case "hands_list":
        return Object.entries(hands).map(([path, h]) => ({
          path,
          name: (metas[path] as { name: string }).name,
          editable: h.editable,
          has_wrist: !!(metas[path] as { wrist?: unknown }).wrist,
        }));
      case "hand_load": {
        const p = a.path as string;
        return { meta: metas[p], ...(await size(`${hands[p].dir}/hand.png`)), editable: hands[p].editable };
      }
      case "hand_image":
        return (await fetch(`${hands[a.path as string].dir}/hand.png`)).arrayBuffer();
      case "hand_save":
        metas[a.path as string] = a.meta;
        return null;
      case "plugin:dialog|open":
        return "C:/demo/demo-phase2.wbproj";
      default:
        return null;
    }
  });
  mockWindows("main");
}
