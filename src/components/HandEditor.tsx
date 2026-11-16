// Trình chỉnh bàn tay: thư viện tay (có sẵn + của bạn), nhập ảnh tay mới (tự xoá nền),
// chấm mũi bút, kẻ đường cắt cổ tay, chỉnh cỡ → lưu vào hand.json.
import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText, type HandDetail, type HandEntry, type HandMeta } from "../api";
import { NumberField, TextField } from "./fields";
import { PALETTE, clayAlpha } from "../theme";

type Mode = "none" | "anchor" | "wrist1" | "wrist2";
type Pt = [number, number];

const PREVIEW_SIDE = 900;

type Props = {
  current: string | null;
  onUse: (path: string) => void;
  onClose: () => void;
  /** Bộ tay đã đổi (lưu/xoá) → editor biên dịch lại xem trước. */
  onChanged: () => void;
};

export default function HandEditor({ current, onUse, onClose, onChanged }: Props) {
  const [list, setList] = useState<HandEntry[]>([]);
  const [path, setPath] = useState<string | null>(current);
  const [detail, setDetail] = useState<HandDetail | null>(null);
  const [meta, setMeta] = useState<HandMeta | null>(null);
  const [img, setImg] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>("none");
  const [pending, setPending] = useState<Pt | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);

  const reloadList = () => api.hands().then(setList).catch((e) => setErr(errorText(e)));
  useEffect(() => {
    void reloadList();
  }, []);
  // tay đang dùng không có trong thư viện (vd. đường dẫn tương đối tự sửa tay) → chọn tay đầu tiên
  useEffect(() => {
    if (list.length && !list.some((h) => h.path === path)) setPath(list[0].path);
  }, [list, path]);

  // nạp bộ tay đang chọn
  useEffect(() => {
    if (!path || !list.some((h) => h.path === path)) return;
    let url: string | null = null;
    let alive = true;
    setErr(null);
    setMode("none");
    setDirty(false);
    Promise.all([api.handLoad(path), api.handImageUrl(path, PREVIEW_SIDE)])
      .then(([d, u]) => {
        url = u;
        if (!alive) return URL.revokeObjectURL(u);
        setDetail(d);
        setMeta(d.meta);
        setImg(u);
      })
      .catch((e) => setErr(errorText(e)));
    return () => {
      alive = false;
      if (url) URL.revokeObjectURL(url);
    };
    // nạp lại khi đổi tay; danh sách đổi (sau lưu) không cần nạp lại
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, list.length > 0]);

  const change = (patch: Partial<HandMeta>) => {
    setMeta((m) => (m ? { ...m, ...patch } : m));
    setDirty(true);
  };

  async function importHand() {
    const file = await open({ title: "Chọn ảnh tay cầm bút", filters: [{ name: "Ảnh", extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof file !== "string") return;
    const name = prompt("Đặt tên cho bộ tay:", "Tay của tôi");
    if (!name) return;
    setBusy("Đang xoá nền và cắt gọn ảnh…");
    try {
      const p = await api.handImport(file, name);
      await reloadList();
      setPath(p);
      setMode("anchor");
    } catch (e) {
      setErr(errorText(e));
    } finally {
      setBusy(null);
    }
  }

  async function duplicate() {
    if (!path) return;
    try {
      const p = await api.handDuplicate(path);
      await reloadList();
      setPath(p);
    } catch (e) {
      setErr(errorText(e));
    }
  }

  async function remove() {
    if (!path || !detail?.editable || !confirm(`Xoá bộ tay "${meta?.name}"?`)) return;
    try {
      await api.handDelete(path);
      setPath(null);
      setDetail(null);
      await reloadList();
      onChanged();
    } catch (e) {
      setErr(errorText(e));
    }
  }

  async function save() {
    if (!path || !meta) return;
    try {
      await api.handSave(path, meta);
      setDirty(false);
      await reloadList();
      onChanged();
    } catch (e) {
      setErr(errorText(e));
    }
  }

  // bấm lên ảnh: đổi sang toạ độ ảnh gốc
  const onPick = (p: Pt) => {
    if (!detail?.editable) return;
    if (mode === "anchor") {
      change({ anchor: p });
      setMode("none");
    } else if (mode === "wrist1") {
      setPending(p);
      setMode("wrist2");
    } else if (mode === "wrist2" && pending) {
      change({ wrist: [pending, p] });
      setPending(null);
      setMode("none");
    }
  };

  const entry = list.find((h) => h.path === path);
  const editable = !!detail?.editable;
  const hint: Record<Mode, string> = {
    none: editable ? "Chọn một thao tác bên phải rồi bấm lên ảnh." : "Tay có sẵn của app — bấm \"Sao chép để chỉnh\" để tạo bản của bạn.",
    anchor: "Bấm vào đúng mũi bút.",
    wrist1: "Bấm điểm thứ nhất của đường cắt (mép cổ tay bên này).",
    wrist2: "Bấm điểm thứ hai (mép cổ tay bên kia). Phần phía xa mũi bút sẽ bị cắt.",
  };

  return (
    <div className="modal-back">
      <div className="modal hand-editor">
        <div className="panel-head">
          <h3>Bàn tay</h3>
          <button onClick={() => (!dirty || confirm("Bỏ các thay đổi chưa lưu?")) && onClose()}>Đóng</button>
        </div>
        <div className="he-body">
          <aside className="he-list">
            <button className="primary" onClick={importHand} disabled={!!busy}>
              ＋ Nhập ảnh tay…
            </button>
            <ol className="list">
              {list.map((h) => (
                <li
                  key={h.path}
                  className={h.path === path ? "active" : ""}
                  onClick={() => (!dirty || confirm("Bỏ các thay đổi chưa lưu?")) && setPath(h.path)}
                >
                  <span className="grow">
                    {h.name}
                    <small>
                      {h.editable ? "của bạn" : "có sẵn"}
                      {h.path === current && " · đang dùng"}
                    </small>
                  </span>
                </li>
              ))}
            </ol>
          </aside>

          <section className="he-canvas">
            {busy && <p className="hint">{busy}</p>}
            {img && detail && meta && <HandCanvas src={img} detail={detail} meta={meta} pending={pending} picking={mode !== "none"} onPick={onPick} />}
            <p className="hint">{hint[mode]}</p>
            {err && <p className="err">{err}</p>}
          </section>

          <aside className="he-props">
            {meta && (
              <>
                {editable ? (
                  <TextField label="Tên" value={meta.name} onCommit={(name) => change({ name })} />
                ) : (
                  <p>
                    <strong>{meta.name}</strong>
                  </p>
                )}
                {editable && (
                  <>
                    <button className={mode === "anchor" ? "primary" : ""} onClick={() => setMode(mode === "anchor" ? "none" : "anchor")}>
                      ● Chấm mũi bút
                    </button>
                    <button
                      className={mode.startsWith("wrist") ? "primary" : ""}
                      onClick={() => {
                        setPending(null);
                        setMode(mode.startsWith("wrist") ? "none" : "wrist1");
                      }}
                    >
                      ╱ Kẻ đường cổ tay
                    </button>
                    {meta.wrist && <button onClick={() => change({ wrist: null })}>Bỏ đường cổ tay</button>}
                    <NumberField
                      label="Cao khi hiện"
                      value={meta.display_height}
                      min={100}
                      max={3000}
                      step={10}
                      suffix="px"
                      onCommit={(display_height) => change({ display_height })}
                    />
                    <NumberField
                      label="Phóng kiểu cả tay"
                      value={meta.full_scale ?? 1.4}
                      min={0.5}
                      max={3}
                      step={0.05}
                      onCommit={(full_scale) => change({ full_scale })}
                    />
                  </>
                )}
                <p className="hint small">
                  "Cao khi hiện" = chiều cao cả ảnh tay trên khung 1080p. Không có đường cổ tay → kiểu "Bàn tay" dùng nguyên ảnh.
                </p>
                <div className="he-actions">
                  {editable ? (
                    <>
                      <button className="primary" disabled={!dirty} onClick={save}>
                        Lưu
                      </button>
                      <button className="danger" onClick={remove}>
                        Xoá
                      </button>
                    </>
                  ) : (
                    <button onClick={duplicate}>Sao chép để chỉnh</button>
                  )}
                  {path && entry && (
                    <button disabled={dirty || path === current} onClick={() => onUse(path)} title={dirty ? "Lưu trước đã" : ""}>
                      Dùng cho dự án
                    </button>
                  )}
                </div>
              </>
            )}
          </aside>
        </div>
      </div>
    </div>
  );
}

/** Ảnh tay + mũi bút (đỏ) + đường cổ tay (xanh) và vùng sẽ bị cắt (mờ). */
function HandCanvas(props: {
  src: string;
  detail: HandDetail;
  meta: HandMeta;
  pending: Pt | null;
  picking: boolean;
  onPick: (p: Pt) => void;
}) {
  const { src, detail, meta, pending, picking, onPick } = props;
  const ref = useRef<SVGSVGElement>(null);
  const { width: W, height: H } = detail;
  const r = Math.max(W, H) / 90;

  const click = (e: React.MouseEvent) => {
    const svg = ref.current;
    if (!svg || !picking) return;
    const b = svg.getBoundingClientRect();
    const x = ((e.clientX - b.left) / b.width) * W;
    const y = ((e.clientY - b.top) / b.height) * H;
    onPick([Math.round(x), Math.round(y)]);
  };

  // vùng bị cắt: nửa mặt phẳng phía không có mũi bút
  let cut: string | null = null;
  if (meta.wrist) {
    const [a, b] = meta.wrist;
    const d = [b[0] - a[0], b[1] - a[1]];
    const len = Math.hypot(d[0], d[1]) || 1;
    const u = [d[0] / len, d[1] / len];
    let n = [-u[1], u[0]];
    const side = (meta.anchor[0] - a[0]) * n[0] + (meta.anchor[1] - a[1]) * n[1];
    if (side > 0) n = [-n[0], -n[1]];
    const F = (W + H) * 4;
    const p1 = [a[0] - u[0] * F, a[1] - u[1] * F];
    const p2 = [a[0] + u[0] * F, a[1] + u[1] * F];
    cut = [p1, p2, [p2[0] + n[0] * F, p2[1] + n[1] * F], [p1[0] + n[0] * F, p1[1] + n[1] * F]]
      .map((p) => p.join(","))
      .join(" ");
  }

  return (
    <svg
      ref={ref}
      className={`he-svg ${picking ? "picking" : ""}`}
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="xMidYMid meet"
      onClick={click}
    >
      <defs>
        <clipPath id="he-clip">
          <rect width={W} height={H} />
        </clipPath>
        <pattern id="checker" width={r * 4} height={r * 4} patternUnits="userSpaceOnUse">
          <rect width={r * 4} height={r * 4} fill={PALETTE.white} />
          <rect width={r * 2} height={r * 2} fill={PALETTE.beige} />
          <rect x={r * 2} y={r * 2} width={r * 2} height={r * 2} fill={PALETTE.beige} />
        </pattern>
      </defs>
      <rect width={W} height={H} fill="url(#checker)" />
      <image href={src} width={W} height={H} />
      {cut && <polygon points={cut} fill={clayAlpha(0.3)} clipPath="url(#he-clip)" />}
      {meta.wrist && (
        <line
          x1={meta.wrist[0][0]}
          y1={meta.wrist[0][1]}
          x2={meta.wrist[1][0]}
          y2={meta.wrist[1][1]}
          stroke={PALETTE.clay}
          strokeWidth={r / 2.5}
        />
      )}
      {pending && <circle cx={pending[0]} cy={pending[1]} r={r / 1.5} fill={PALETTE.clay} />}
      <circle cx={meta.anchor[0]} cy={meta.anchor[1]} r={r} fill="none" stroke={PALETTE.ink} strokeWidth={r / 3} />
      <circle cx={meta.anchor[0]} cy={meta.anchor[1]} r={r / 4} fill={PALETTE.ink} />
    </svg>
  );
}
