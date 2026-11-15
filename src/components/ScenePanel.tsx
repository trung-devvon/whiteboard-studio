// Cột trái: danh sách scene + danh sách element của scene đang chọn.
import type { Element } from "../bindings/model";
import { useEditor } from "../store";

function label(e: Element) {
  if (e.kind.type === "text") return `✎ ${e.kind.text.split("\n")[0].slice(0, 24) || "(chữ trống)"}`;
  const name = e.kind.src.split(/[\\/]/).pop() ?? e.kind.src;
  return `🖼 ${name.replace(/-[0-9a-f]{16}(\.\w+)$/, "$1")}`;
}

export default function ScenePanel({ sceneTimes }: { sceneTimes: number[] }) {
  const project = useEditor((s) => s.project);
  const sceneIndex = useEditor((s) => s.sceneIndex);
  const selectedId = useEditor((s) => s.selectedId);
  const { selectScene, addScene, deleteScene, moveScene, select, moveElement } = useEditor.getState();
  if (!project) return null;
  const scene = project.scenes[Math.min(sceneIndex, project.scenes.length - 1)];

  return (
    <aside className="panel left">
      <div className="panel-head">
        <h3>Cảnh</h3>
        <button onClick={addScene} title="Thêm cảnh">
          ＋
        </button>
      </div>
      <ol className="list scenes">
        {project.scenes.map((s, i) => (
          <li key={s.id} className={i === sceneIndex ? "active" : ""} onClick={() => selectScene(i)}>
            <span className="grow">
              {i + 1}. {s.name || `Cảnh ${i + 1}`}
              <small>
                {s.elements.length} đối tượng
                {sceneTimes[i] !== undefined && ` · ${(sceneTimes[i] / 1000).toFixed(1)}s`}
              </small>
            </span>
            {i === sceneIndex && (
              <span className="row-actions" onClick={(e) => e.stopPropagation()}>
                <button onClick={() => moveScene(i, -1)} disabled={i === 0} title="Lên">
                  ↑
                </button>
                <button onClick={() => moveScene(i, 1)} disabled={i === project.scenes.length - 1} title="Xuống">
                  ↓
                </button>
                <button
                  onClick={() => confirm(`Xoá "${s.name || `Cảnh ${i + 1}`}"?`) && deleteScene(i)}
                  disabled={project.scenes.length <= 1}
                  title="Xoá cảnh"
                >
                  ✕
                </button>
              </span>
            )}
          </li>
        ))}
      </ol>

      <div className="panel-head">
        <h3>Thứ tự vẽ</h3>
      </div>
      {scene.elements.length === 0 && <p className="hint">Cảnh trống — thêm ảnh hoặc chữ ở thanh trên.</p>}
      <ol className="list elements">
        {scene.elements.map((e, i) => (
          <li key={e.id} className={e.id === selectedId ? "active" : ""} onClick={() => select(e.id)}>
            <span className="grow">
              {i + 1}. {label(e)}
            </span>
            {e.id === selectedId && (
              <span className="row-actions" onClick={(ev) => ev.stopPropagation()}>
                <button onClick={() => moveElement(e.id, -1)} disabled={i === 0} title="Vẽ sớm hơn">
                  ↑
                </button>
                <button onClick={() => moveElement(e.id, 1)} disabled={i === scene.elements.length - 1} title="Vẽ muộn hơn">
                  ↓
                </button>
              </span>
            )}
          </li>
        ))}
      </ol>
    </aside>
  );
}
