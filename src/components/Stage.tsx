// Sân khấu: khung Player (canvas WASM) + lớp Konva để chọn/kéo/đổi cỡ element.
// Toạ độ trong dự án tính theo khung tham chiếu 1920×1080; ở đây nhân `scale` để hiển thị.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Layer, Rect, Stage as KStage, Text as KText, Transformer } from "react-konva";
import type Konva from "konva";
import type { Element } from "../bindings/model";
import type { Preview } from "../player";
import { REF_H, REF_W, boxOf, useEditor } from "../store";

type Props = {
  preview: Preview | null;
  time: number;
  playing: boolean;
  busy: boolean;
  error: string | null;
};

export default function Stage({ preview, time, playing, busy, error }: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 960, h: 540 });

  // co khung theo chỗ trống, giữ 16:9
  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => {
      const { width, height } = e.contentRect;
      const s = Math.max(0.1, Math.min((width - 24) / REF_W, (height - 24) / REF_H));
      setSize({ w: Math.round(REF_W * s), h: Math.round(REF_H * s) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const c = canvas.current;
    if (!c || !preview) return;
    const ctx = c.getContext("2d");
    if (ctx) preview.draw(ctx, time);
  }, [preview, time]);

  const scale = size.w / REF_W;
  return (
    <div className="stage-wrap" ref={wrap}>
      <div className="stage" style={{ width: size.w, height: size.h }}>
        <canvas
          ref={canvas}
          width={preview?.width ?? 960}
          height={preview?.height ?? 540}
          style={{ width: size.w, height: size.h }}
        />
        {!playing && <Overlay scale={scale} width={size.w} height={size.h} />}
        {busy && <div className="stage-badge">Đang phân tích…</div>}
        {error && <div className="stage-error">{error}</div>}
      </div>
    </div>
  );
}

function Overlay({ scale, width, height }: { scale: number; width: number; height: number }) {
  const project = useEditor((s) => s.project);
  const sceneIndex = useEditor((s) => s.sceneIndex);
  const selectedId = useEditor((s) => s.selectedId);
  const select = useEditor((s) => s.select);
  const updateElement = useEditor((s) => s.updateElement);
  const tr = useRef<Konva.Transformer>(null);
  const stage = useRef<Konva.Stage>(null);
  const scene = project?.scenes[Math.min(sceneIndex, (project?.scenes.length ?? 1) - 1)];

  useEffect(() => {
    const t = tr.current;
    if (!t) return;
    const node = selectedId ? stage.current?.findOne(`#${selectedId}`) : null;
    t.nodes(node ? [node] : []);
    t.getLayer()?.batchDraw();
  }, [selectedId, scene]);

  const commit = (e: Element, node: Konva.Node) => {
    const w = Math.max(20, (node.width() * node.scaleX()) / scale);
    const h = Math.max(20, (node.height() * node.scaleY()) / scale);
    node.scale({ x: 1, y: 1 });
    updateElement(e.id, {
      transform: { x: Math.round(node.x() / scale), y: Math.round(node.y() / scale), w: Math.round(w), h: Math.round(h) },
    });
  };

  return (
    <KStage
      ref={stage}
      width={width}
      height={height}
      className="stage-overlay"
      onMouseDown={(ev) => ev.target === ev.target.getStage() && select(null)}
    >
      <Layer>
        {scene?.elements.map((e, i) => {
          const b = boxOf(e);
          const sel = e.id === selectedId;
          return (
            <Rect
              key={e.id}
              id={e.id}
              x={b.x * scale}
              y={b.y * scale}
              width={b.w * scale}
              height={b.h * scale}
              stroke={sel ? "#2f6fed" : "rgba(47,111,237,0.35)"}
              dash={sel ? undefined : [6, 4]}
              strokeWidth={1}
              strokeScaleEnabled={false}
              fill="rgba(0,0,0,0.001)"
              draggable
              onMouseDown={() => select(e.id)}
              onDragEnd={(ev) => commit(e, ev.target)}
              onTransformEnd={(ev) => commit(e, ev.target)}
              name={`#${i + 1}`}
            />
          );
        })}
        {scene?.elements.map((e, i) => {
          const b = boxOf(e);
          return (
            <KText
              key={`n-${e.id}`}
              x={b.x * scale + 4}
              y={b.y * scale + 4}
              text={`${i + 1}`}
              fontSize={11}
              fill="#2f6fed"
              listening={false}
            />
          );
        })}
        <Transformer
          ref={tr}
          rotateEnabled={false}
          keepRatio={false}
          ignoreStroke
          boundBoxFunc={(oldB, newB) => (newB.width < 10 || newB.height < 10 ? oldB : newB)}
        />
      </Layer>
    </KStage>
  );
}
