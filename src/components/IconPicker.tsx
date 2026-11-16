// Chọn icon có sẵn (Lucide, ISC): gõ tiếng Việt hoặc tiếng Anh, bấm để thêm vào cảnh.
import { useEffect, useState } from "react";
import { api, errorText, type IconHit } from "../api";
import { PALETTE } from "../theme";

const CHIPS = ["mũi tên", "người", "tiền", "biểu đồ", "học", "thời gian", "máy tính", "nhà", "ý tưởng", "cảnh báo"];

export default function IconPicker({ onPick, onClose }: { onPick: (icon: IconHit) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<IconHit[]>([]);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    const t = setTimeout(() => {
      api
        .icons(query, 160)
        .then((h) => {
          setHits(h);
          setErr(null);
        })
        .catch((e) => setErr(errorText(e)));
    }, 150);
    return () => clearTimeout(t);
  }, [query]);

  return (
    <div className="modal-back" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal icon-picker">
        <div className="panel-head">
          <h3>Thêm icon</h3>
          <button onClick={onClose}>Đóng</button>
        </div>
        <input
          className="search"
          autoFocus
          placeholder="Tìm icon: mũi tên, bóng đèn, người, arrow, star…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && onClose()}
        />
        <div className="chips">
          {CHIPS.map((c) => (
            <button key={c} className={query === c ? "primary" : ""} onClick={() => setQuery(c)}>
              {c}
            </button>
          ))}
        </div>
        {err && <p className="err">{err}</p>}
        <div className="icon-grid">
          {hits.map((h) => (
            <button key={h.name} title={h.name} onClick={() => onPick(h)}>
              <img alt={h.name} src={`data:image/svg+xml;utf8,${encodeURIComponent(h.svg.replace(/currentColor/g, PALETTE.ink))}`} />
              <span>{h.name}</span>
            </button>
          ))}
          {hits.length === 0 && !err && <p className="hint">Không tìm thấy — thử từ khác (hoặc tiếng Anh).</p>}
        </div>
        <p className="hint small">Icon: Lucide (giấy phép ISC). Icon được vẽ theo đúng nét của nó.</p>
      </div>
    </div>
  );
}
