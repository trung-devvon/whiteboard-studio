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
import type { ElementInfo, HandEntry, NarrationInfo } from "../api";
import { VOICES } from "./VoiceDialog";
import { boxOf, findElement, useEditor } from "../store";
import { CONTENT } from "../theme";
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

/** Giọng đọc: trạng thái từng scene + hành động (tạo giọng, mở cài đặt). */
export type VoiceProps = {
  narration: NarrationInfo[];
  generating: boolean;
  onGenerate: () => void;
  onSettings: () => void;
};

type Props = { info: ElementInfo[]; hands: HandEntry[]; onManageHands: () => void; voice: VoiceProps };

export default function Inspector({ info, hands, onManageHands, voice }: Props) {
  const project = useEditor((s) => s.project);
  const selectedId = useEditor((s) => s.selectedId);
  const el = findElement(project, selectedId);
  return (
    <aside className="panel right">
      {el ? <ElementProps el={el} info={info.find((i) => i.id === el.id)} /> : <SceneProps hands={hands} onManageHands={onManageHands} voice={voice} />}
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
          <ColorField label="Màu" value={k.color ?? CONTENT.ink} onChange={(color) => updateKind(el.id, { color })} />
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
          <TintField value={k.tint ?? null} onChange={(tint) => updateKind(el.id, { tint })} />
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

function SceneProps({ hands, onManageHands, voice }: { hands: HandEntry[]; onManageHands: () => void; voice: VoiceProps }) {
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
          value={scene.background ?? set.background ?? CONTENT.background}
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

      <h4>Lời thoại</h4>
      <section>
        <TextField
          label=""
          multiline
          value={scene.narration ?? ""}
          placeholder="Gõ lời đọc cho cảnh này…"
          onCommit={(v) => updateScene(i, { narration: v.trim() ? v : null })}
        />
        <p className="hint small">
          Gõ <code>[1]</code>, <code>[2]</code>… vào chỗ muốn đối tượng số 1, 2 (thứ tự vẽ bên trái) bắt đầu vẽ đúng lúc đọc tới.
        </p>
        <NarrationState info={voice.narration.find((n) => n.scene === scene.id)} voice={voice} />
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
        <ColorField label="Nền mặc định" value={set.background ?? CONTENT.background} onChange={(background) => updateSettings({ background })} />
        <NumberField label="FPS xuất" value={set.fps ?? 30} min={10} max={60} onCommit={(fps) => updateSettings({ fps })} />
        <SelectField
          label="Giọng đọc"
          value={set.voice?.name ?? VOICES[0][0]}
          options={VOICES}
          onChange={(name) => updateSettings({ voice: { name, rate: set.voice?.rate ?? 1 } })}
        />
        <NumberField
          label="Tốc độ đọc"
          value={(set.voice?.rate ?? 1) * 100}
          min={50}
          max={200}
          step={5}
          suffix="%"
          onCommit={(v) => updateSettings({ voice: { name: set.voice?.name ?? VOICES[0][0], rate: v / 100 } })}
        />
        <button className="link" onClick={voice.onSettings}>
          Cài đặt giọng đọc (key Azure)…
        </button>
        <NumberField
          label="Tiếng bút"
          value={(set.sound?.pen_volume ?? 1) * 100}
          min={0}
          max={200}
          step={10}
          suffix="%"
          onCommit={(v) => updateSettings({ sound: { pen_volume: v / 100 } })}
        />
      </section>
    </>
  );
}

/** Trạng thái giọng đọc của scene + nút tạo giọng. */
function NarrationState({ info, voice }: { info?: NarrationInfo; voice: VoiceProps }) {
  if (!info || info.state === "none") return null;
  const missing = voice.narration.filter((n) => n.state === "missing").length;
  return (
    <div className="narration-state">
      {info.state === "ready" ? (
        <span>✓ Đã có giọng đọc · {(info.duration_ms / 1000).toFixed(1)} s</span>
      ) : (
        <span>Chưa tạo giọng cho lời thoại này</span>
      )}
      {info.unknown_markers.length > 0 && (
        <span className="err">Không có đối tượng số {info.unknown_markers.join(", ")} trong cảnh</span>
      )}
      {missing > 0 && (
        <button className="primary" onClick={voice.onGenerate} disabled={voice.generating}>
          {voice.generating ? "Đang tạo giọng…" : missing > 1 ? `Tạo giọng đọc (${missing} cảnh)` : "Tạo giọng đọc"}
        </button>
      )}
    </div>
  );
}

/** Đổi màu ảnh: tắt = giữ màu gốc; bật = mọi nét thành một màu (hợp cho icon, mũi tên, ảnh nét). */
function TintField({ value, onChange }: { value: string | null; onChange: (v: string | null) => void }) {
  return (
    <div className="tint">
      <label className="field">
        <span>Đổi màu nét</span>
        <input
          type="checkbox"
          checked={value !== null}
          onChange={(e) => onChange(e.target.checked ? CONTENT.swatches[1] : null)}
        />
      </label>
      {value !== null && (
        <div className="swatches">
          {CONTENT.swatches.map((c) => (
            <button
              key={c}
              className={`swatch ${value.toUpperCase() === c ? "on" : ""}`}
              style={{ background: c }}
              title={c}
              aria-label={`Màu ${c}`}
              onClick={() => onChange(c)}
            />
          ))}
          <input type="color" value={value.slice(0, 7)} onChange={(e) => onChange(e.target.value.toUpperCase())} title="Màu khác…" />
        </div>
      )}
    </div>
  );
}
