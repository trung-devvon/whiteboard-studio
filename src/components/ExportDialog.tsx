// Hộp thoại xuất video: chọn file, độ phân giải → tiến độ, huỷ, mở thư mục khi xong.
import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { Project } from "../bindings/model";
import { api, errorText } from "../api";

type Phase =
  | { k: "idle" }
  | { k: "preparing" }
  | { k: "running"; frame: number; total: number }
  | { k: "done"; path: string; seconds: number; encoder: string }
  | { k: "error"; message: string };

export default function ExportDialog({ root, project, onClose }: { root: string; project: Project; onClose: () => void }) {
  const projectW = project.settings?.width ?? 1920;
  const [width, setWidth] = useState(projectW);
  const [phase, setPhase] = useState<Phase>({ k: "idle" });
  const running = phase.k === "preparing" || phase.k === "running";

  async function start() {
    const name = root.split(/[\\/]/).pop()?.replace(/\.wbproj$/i, "") || "video";
    const out = await save({
      defaultPath: `${root}/../${name}.mp4`,
      filters: [{ name: "Video MP4", extensions: ["mp4"] }],
    });
    if (!out) return;
    setPhase({ k: "preparing" });
    try {
      await api.exportVideo(root, project, out, width === projectW ? null : width, (e) => {
        if (e.type === "preparing") setPhase({ k: "preparing" });
        else if (e.type === "progress") setPhase({ k: "running", frame: e.frame, total: e.total });
        else setPhase({ k: "done", path: e.path, seconds: e.seconds, encoder: e.encoder });
      });
    } catch (e) {
      setPhase({ k: "error", message: errorText(e) });
    }
  }

  const pct = phase.k === "running" ? (phase.frame / Math.max(1, phase.total)) * 100 : phase.k === "done" ? 100 : 0;
  return (
    <div className="modal-back">
      <div className="modal">
        <h3>Xuất video MP4</h3>
        <label className="field">
          <span>Độ phân giải</span>
          <select value={width} disabled={running} onChange={(e) => setWidth(Number(e.target.value))}>
            {[...new Set([projectW, 1920, 1280, 960])].map((w) => (
              <option key={w} value={w}>
                {w}×{Math.round((w * (project.settings?.height ?? 1080)) / projectW)}
                {w === projectW ? " (dự án)" : ""}
              </option>
            ))}
          </select>
        </label>

        {phase.k !== "idle" && (
          <div className="progress">
            <div style={{ width: `${pct}%` }} />
          </div>
        )}
        <p className="status">
          {phase.k === "preparing" && "Đang phân tích ảnh và chuẩn bị…"}
          {phase.k === "running" && `Khung ${phase.frame}/${phase.total} (${pct.toFixed(0)}%)`}
          {phase.k === "done" && `Xong trong ${phase.seconds.toFixed(1)} s (encoder ${phase.encoder}).`}
          {phase.k === "error" && <span className="err">{phase.message}</span>}
        </p>

        <div className="modal-actions">
          {phase.k === "done" && <button onClick={() => revealItemInDir(phase.path)}>Mở thư mục</button>}
          {running ? (
            <button className="danger" onClick={() => api.exportCancel()}>
              Huỷ
            </button>
          ) : (
            <>
              <button onClick={onClose}>Đóng</button>
              <button className="primary" onClick={start}>
                {phase.k === "done" || phase.k === "error" ? "Xuất lại…" : "Chọn file & xuất…"}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
