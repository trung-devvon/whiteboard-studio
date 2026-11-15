// CHỈ DÙNG KHI PHÁT TRIỂN: chạy giao diện trong trình duyệt thường (`npm run dev`, không có Tauri)
// để xem/thử bố cục. Lệnh Rust được giả lập; Player dùng gói biên dịch thật tạo sẵn bằng:
//   wb bundle tests/fixtures/projects/demo-phase2.json -o target/preview-bundle.bin
// Thay đổi trên giao diện không biên dịch lại được (khung Player giữ nguyên).
import { mockIPC } from "@tauri-apps/api/mocks";

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

  mockIPC((cmd) => {
    switch (cmd) {
      case "project_open":
      case "project_create":
        return { root: "C:/demo/demo-phase2.wbproj", project, recovery: null };
      case "compile_preview":
        return packed.buffer;
      case "builtin_hands":
        return [
          { path: "builtin:hands/pen-brown", name: "Bút nâu – tay phải" },
          { path: "builtin:hands/pencil-green", name: "Bút chì xanh" },
        ];
      case "plugin:dialog|open":
        return "C:/demo/demo-phase2.wbproj";
      case "plugin:dialog|save":
        return null;
      default:
        return null;
    }
  });
}
