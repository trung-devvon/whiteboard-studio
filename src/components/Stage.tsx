// Sân khấu: khung Player (canvas WASM) + lớp Konva để chọn/kéo/đổi cỡ element.
// Toạ độ trong dự án tính theo khung tham chiếu 1920×1080; ở đây nhân `scale` để hiển thị.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Label, Layer, Line, Rect, Stage as KStage, Tag, Text as KText, Transformer } from "react-konva";
import type Konva from "konva";
import type { Element } from "../bindings/model";
import type { Preview } from "../player";
import { REF_H, REF_W, boxOf, useEditor } from "../store";
import { PALETTE, inkAlpha } from "../theme";

/** Khoảng cách (px màn hình) để hút vào đường gióng. */
const SNAP_PX = 6;

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

  // bắt dính: cạnh trái/giữa/phải (trên/giữa/dưới) của element đang kéo hút vào mép + giữa khung
  // và cạnh/giữa các element khác; giữ Alt để tắt
  const [guides, setGuides] = useState<{ v: number[]; h: number[] }>({ v: [], h: [] });
  const snap = (e: Element, node: Konva.Node, alt: boolean) => {
    if (alt || !scene) return setGuides({ v: [], h: [] });
    const w = node.width() * node.scaleX();
    const h = node.height() * node.scaleY();
    const vs = [0, width / 2, width];
    const hs = [0, height / 2, height];
    for (const o of scene.elements) {
      if (o.id === e.id) continue;
      const b = boxOf(o);
      vs.push(b.x * scale, (b.x + b.w / 2) * scale, (b.x + b.w) * scale);
      hs.push(b.y * scale, (b.y + b.h / 2) * scale, (b.y + b.h) * scale);
    }
    const best = (pos: number, size: number, targets: number[]) => {
      let r: { d: number; at: number } | null = null;
      for (const edge of [pos, pos + size / 2, pos + size])
        for (const t of targets) {
          const d = t - edge;
          if (Math.abs(d) <= SNAP_PX && (!r || Math.abs(d) < Math.abs(r.d))) r = { d, at: t };
        }
      return r;
    };
    const bx = best(node.x(), w, vs);
    const by = best(node.y(), h, hs);
    if (bx) node.x(node.x() + bx.d);
    if (by) node.y(node.y() + by.d);
    setGuides({ v: bx ? [bx.at] : [], h: by ? [by.at] : [] });
  };

  const commit = (e: Element, node: Konva.Node) => {
    setGuides({ v: [], h: [] });
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
        {/* quầng trắng dưới mọi nét vẽ đè lên hình (skill whiteboard-ui, quy tắc 5) */}
        {scene?.elements.map((e) => {
          const b = boxOf(e);
          return (
            <Rect
              key={`halo-${e.id}`}
              x={b.x * scale}
              y={b.y * scale}
              width={b.w * scale}
              height={b.h * scale}
              stroke={PALETTE.white}
              strokeWidth={3}
              strokeScaleEnabled={false}
              listening={false}
            />
          );
        })}
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
              stroke={sel ? PALETTE.clay : PALETTE.ink}
              dash={sel ? undefined : [6, 4]}
              strokeWidth={sel ? 2 : 1}
              strokeScaleEnabled={false}
              fill={inkAlpha(0.001)}
              draggable
              onMouseDown={() => select(e.id)}
              onDragMove={(ev) => snap(e, ev.target, ev.evt.altKey)}
              onDragEnd={(ev) => commit(e, ev.target)}
              onTransformEnd={(ev) => commit(e, ev.target)}
              name={`#${i + 1}`}
            />
          );
        })}
        {/* số thứ tự: chữ trắng trên tấm nền mực, không bao giờ nằm trần trên hình */}
        {scene?.elements.map((e, i) => {
          const b = boxOf(e);
          const sel = e.id === selectedId;
          return (
            <Label key={`n-${e.id}`} x={b.x * scale + 4} y={b.y * scale + 4} listening={false}>
              <Tag fill={sel ? PALETTE.clay : PALETTE.ink} stroke={PALETTE.white} strokeWidth={1} cornerRadius={3} />
              <KText text={`${i + 1}`} fontSize={11} fontStyle="bold" fill={PALETTE.white} padding={3} />
            </Label>
          );
        })}
        {guides.v.map((x) => [
          <Line key={`vh${x}`} points={[x, 0, x, height]} stroke={PALETTE.white} strokeWidth={3} listening={false} />,
          <Line key={`v${x}`} points={[x, 0, x, height]} stroke={PALETTE.clay} strokeWidth={1} dash={[4, 4]} listening={false} />,
        ])}
        {guides.h.map((y) => [
          <Line key={`hh${y}`} points={[0, y, width, y]} stroke={PALETTE.white} strokeWidth={3} listening={false} />,
          <Line key={`h${y}`} points={[0, y, width, y]} stroke={PALETTE.clay} strokeWidth={1} dash={[4, 4]} listening={false} />,
        ])}
        <Transformer
          ref={tr}
          rotateEnabled={false}
          keepRatio={false}
          ignoreStroke
          borderStroke={PALETTE.clay}
          anchorStroke={PALETTE.clay}
          anchorFill={PALETTE.white}
          anchorSize={8}
          anchorCornerRadius={2}
          boundBoxFunc={(oldB, newB) => (newB.width < 10 || newB.height < 10 ? oldB : newB)}
        />
      </Layer>
    </KStage>
  );
}
