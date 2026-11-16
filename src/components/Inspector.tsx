// Cột phải: thuộc tính element đang chọn; không chọn gì → thuộc tính cảnh + dự án.
import type {
  BackgroundMode,
  DrawStyle,
  Element,
  HandMode,
  PipelineChoice,
  TextAlign,
  Timing,
} from "../bindings/model";
import type { ElementInfo, HandEntry } from "../api";
import { boxOf, findElement, useEditor } from "../store";
import { ColorField, NumberField, SelectField, TextField } from "./fields";

const STYLES: [DrawStyle, string][] = [
  ["auto", "Tự chọn"],
  ["sketch", "Chỉ nét"],
  ["sketch_then_color", "Nét rồi tô màu"],
  ["color_brush", "Tô màu bằng cọ"],
];
const PIPELINES: [PipelineChoice, string][] = [
  ["auto", "Tự nhận"],
  ["line_art", "Nét vẽ (line-art)"],
  ["color", "Ảnh màu / clipart"],
  ["photo", "Ảnh chụp"],
  ["vector", "Vector (SVG)"],
];
const BACKGROUNDS: [BackgroundMode, string][] = [
  ["auto", "Tự quyết"],
  ["keep", "Giữ nền"],
  ["remove", "Xoá nền đồng màu"],
];
const ALIGNS: [TextAlign, string][] = [
  ["left", "Trái"],
  ["center", "Giữa"],
  ["right", "Phải"],
];
const HAND_MODES: [HandMode, string][] = [
  ["wrist", "Bàn tay (tới cổ tay)"],
  ["full", "Cả cánh tay"],
];

type Props = { info: ElementInfo[]; hands: HandEntry[]; onManageHands: () => void };

export default function Inspector({ info, hands, onManageHands }: Props) {
  const project = useEditor((s) => s.project);
  const selectedId = useEditor((s) => s.selectedId);
  const el = findElement(project, selectedId);
  return (
    <aside className="panel right">
      {el ? <ElementProps el={el} info={info.find((i) => i.id === el.id)} /> : <SceneProps hands={hands} onManageHands={onManageHands} />}
    </aside>
  );
}

function ElementProps({ el, info }: { el: Element; info?: ElementInfo }) {
  const { updateElement, updateKind, deleteElement } = useEditor.getState();
  const b = boxOf(el);
  const setBox = (k: "x" | "y" | "w" | "h", v: number) => updateElement(el.id, { transform: { ...b, [k]: v } });
  const timing: Required<Timing> = {
    gap_before_ms: el.timing?.gap_before_ms ?? 300,
    duration_ms: el.timing?.duration_ms ?? null,
    settle_ms: el.timing?.settle_ms ?? 300,
  };
  const setTiming = (patch: Partial<Timing>) => updateElement(el.id, { timing: { ...timing, ...patch } });
  const k = el.kind;

  return (
    <>
      <div className="panel-head">
        <h3>{k.type === "text" ? "Chữ viết tay" : "Ảnh"}</h3>
        <button className="danger" onClick={() => deleteElement(el.id)} title="Xoá (Delete)">
          Xoá
        </button>
      </div>

      {k.type === "text" ? (
        <section>
          <TextField label="Nội dung" multiline value={k.text} onCommit={(text) => updateKind(el.id, { text })} />
          <NumberField label="Cỡ chữ" value={k.size ?? 72} min={8} max={600} onCommit={(size) => updateKind(el.id, { size })} />
          <ColorField label="Màu" value={k.color ?? "#1D1D1F"} onChange={(color) => updateKind(el.id, { color })} />
          <SelectField label="Căn lề" value={k.align ?? "left"} options={ALIGNS} onChange={(align) => updateKind(el.id, { align })} />
          <NumberField
            label="Giãn dòng"
            value={k.line_height ?? 1.35}
            min={0.8}
            max={3}
            step={0.05}
            onCommit={(line_height) => updateKind(el.id, { line_height })}
          />
        </section>
      ) : (
        <section>
          <p className="path" title={k.src}>
            {k.src.split(/[\\/]/).pop()}
          </p>
          <SelectField label="Kiểu vẽ" value={k.style ?? "auto"} options={STYLES} onChange={(style) => updateKind(el.id, { style })} />
          <SelectField
            label="Loại ảnh"
            value={k.pipeline ?? "auto"}
            options={PIPELINES}
            onChange={(pipeline) => updateKind(el.id, { pipeline })}
          />
          <SelectField
            label="Nền"
            value={k.background ?? "auto"}
            options={BACKGROUNDS}
            onChange={(background) => updateKind(el.id, { background })}
          />
          <NumberField
            label="Cỡ cọ tô"
            value={(k.params?.brush_size ?? 0.025) * 100}
            min={0.5}
            max={10}
            step={0.1}
            suffix="%"
            onCommit={(v) => updateKind(el.id, { params: { ...k.params, brush_size: v / 100 } })}
          />
        </section>
      )}

      <h4>Vị trí (khung 1920×1080)</h4>
      <section className="grid2">
        <NumberField label="X" value={b.x} onCommit={(v) => setBox("x", v)} />
        <NumberField label="Y" value={b.y} onCommit={(v) => setBox("y", v)} />
        <NumberField label="Rộng" value={b.w} min={10} onCommit={(v) => setBox("w", v)} />
        <NumberField label="Cao" value={b.h} min={10} onCommit={(v) => setBox("h", v)} />
      </section>

      <h4>Thời gian</h4>
      <section>
        <NumberField label="Nghỉ trước" value={timing.gap_before_ms} min={0} step={50} suffix="ms" onCommit={(v) => setTiming({ gap_before_ms: v })} />
        <NumberField
          label="Thời lượng vẽ"
          value={timing.duration_ms ? timing.duration_ms / 1000 : null}
          min={0}
          step={0.5}
          suffix="s (trống = tự)"
          onCommit={(v) => setTiming({ duration_ms: v > 0 ? Math.round(v * 1000) : null })}
        />
        <NumberField label="Hoàn thiện" value={timing.settle_ms} min={0} step={50} suffix="ms" onCommit={(v) => setTiming({ settle_ms: v })} />
      </section>

      {info && (
        <p className="analysis">
          {info.kind} · {info.style} · {info.layers.join(" → ")}
          <br />
          {info.strokes} nét · phủ {(info.coverage * 100).toFixed(1)}% · phân tích {info.analyze_ms} ms
        </p>
      )}
    </>
  );
}

function SceneProps({ hands, onManageHands }: { hands: HandEntry[]; onManageHands: () => void }) {
  const project = useEditor((s) => s.project);
  const sceneIndex = useEditor((s) => s.sceneIndex);
  const { renameScene, updateScene, updateSettings } = useEditor.getState();
  if (!project) return null;
  const i = Math.min(sceneIndex, project.scenes.length - 1);
  const scene = project.scenes[i];
  const set = project.settings ?? {};
  const handOptions: [string, string][] = [["", "Không có tay"], ...hands.map((h): [string, string] => [h.path, h.name])];
  if (set.hand && !hands.some((h) => h.path === set.hand)) handOptions.push([set.hand, set.hand]);

  return (
    <>
      <div className="panel-head">
        <h3>Cảnh {i + 1}</h3>
      </div>
      <section>
        <TextField label="Tên" value={scene.name ?? ""} onCommit={(v) => renameScene(i, v)} />
        <ColorField
          label="Màu nền"
          value={scene.background ?? set.background ?? "#FFFFFF"}
          onChange={(background) => updateScene(i, { background })}
        />
        <NumberField
          label="Giữ hình cuối"
          value={(scene.tail_ms ?? 1500) / 1000}
          min={0}
          step={0.5}
          suffix="s"
          onCommit={(v) => updateScene(i, { tail_ms: Math.round(v * 1000) })}
        />
      </section>

      <div className="panel-head">
        <h3>Dự án</h3>
      </div>
      <section>
        <SelectField label="Bàn tay" value={set.hand ?? ""} options={handOptions} onChange={(hand) => updateSettings({ hand: hand || null })} />
        <button className="link" onClick={onManageHands}>
          Quản lý / thêm bàn tay…
        </button>
        <SelectField label="Kiểu tay" value={set.hand_mode ?? "wrist"} options={HAND_MODES} onChange={(hand_mode) => updateSettings({ hand_mode })} />
        <NumberField
          label="Cỡ tay"
          value={(set.hand_scale ?? 1) * 100}
          min={20}
          max={300}
          step={5}
          suffix="%"
          onCommit={(v) => updateSettings({ hand_scale: v / 100 })}
        />
        <NumberField
          label="Tốc độ vẽ"
          value={set.draw_speed ?? 1400}
          min={200}
          max={10000}
          step={100}
          suffix="px/s"
          onCommit={(draw_speed) => updateSettings({ draw_speed })}
        />
        <ColorField label="Nền mặc định" value={set.background ?? "#FFFFFF"} onChange={(background) => updateSettings({ background })} />
        <NumberField label="FPS xuất" value={set.fps ?? 30} min={10} max={60} onCommit={(fps) => updateSettings({ fps })} />
      </section>
    </>
  );
}
