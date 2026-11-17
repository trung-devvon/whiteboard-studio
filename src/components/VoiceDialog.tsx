// Cài đặt giọng đọc: nhập key Azure Speech (cất trong kho mật khẩu Windows), nghe thử giọng.
// App không bao giờ hiện lại key đã lưu — chỉ biết "đã cài" + vùng.
import { useEffect, useState } from "react";
import { api, errorText, type VoiceStatus } from "../api";
import { playOnce } from "../audio";
import type { VoiceSettings } from "../bindings/model";

export const VOICES: [string, string][] = [
  ["vi-VN-HoaiMyNeural", "HoaiMy (nữ)"],
  ["vi-VN-NamMinhNeural", "NamMinh (nam)"],
];

const SAMPLE = "Xin chào các bạn, đây là giọng đọc thử cho video bảng trắng.";

export default function VoiceDialog({ voice, onClose }: { voice: VoiceSettings; onClose: () => void }) {
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [key, setKey] = useState("");
  const [region, setRegion] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .voiceStatus()
      .then((s) => {
        setStatus(s);
        setRegion(s.region ?? "");
      })
      .catch((e) => setErr(errorText(e)));
  }, []);

  const run = (label: string, f: () => Promise<void>) => async () => {
    setBusy(label);
    setErr(null);
    setMsg(null);
    try {
      await f();
    } catch (e) {
      setErr(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const save = run("Đang thử key…", async () => {
    const s = await api.voiceSave(key, region);
    setStatus(s);
    setKey("");
    setMsg("Key dùng được — đã cất vào kho mật khẩu của Windows.");
  });
  const clear = run("Đang xoá…", async () => {
    setStatus(await api.voiceClear());
    setMsg("Đã xoá key lưu trong app.");
  });
  const listen = run("Đang tạo giọng…", async () => {
    await playOnce(await api.voicePreview(voice.name ?? VOICES[0][0], voice.rate ?? 1, SAMPLE));
  });

  return (
    <div className="modal-back" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="modal voice-dialog">
        <h3>Cài đặt giọng đọc</h3>
        <p className="hint small">
          Dùng dịch vụ <strong>Azure AI Speech</strong> bằng key của bạn (gói Free F0: 0,5 triệu ký tự mỗi tháng). Lấy key ở
          portal.azure.com → resource Speech → <em>Keys and Endpoint</em>.
        </p>

        <div className="voice-state">
          {status === null
            ? "Đang kiểm tra…"
            : status.configured
              ? `Đã cài key · vùng ${status.region}${status.source === "env" ? " · lấy từ file .env (chế độ phát triển)" : ""}`
              : "Chưa có key"}
        </div>

        <label className="field column">
          <span>Key (KEY 1)</span>
          <input
            type="password"
            autoComplete="off"
            value={key}
            placeholder={status?.configured ? "Nhập key mới để thay" : "Dán key vào đây"}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
        <label className="field column">
          <span>Vùng (Location/Region)</span>
          <input value={region} placeholder="vd. japaneast" onChange={(e) => setRegion(e.target.value)} />
        </label>

        {busy && <p className="status">{busy}</p>}
        {msg && <p className="status">{msg}</p>}
        {err && <p className="err">{err}</p>}

        <div className="modal-actions">
          {status?.source === "keyring" && (
            <button className="danger" onClick={clear} disabled={!!busy}>
              Xoá key
            </button>
          )}
          <button onClick={listen} disabled={!!busy || !status?.configured} title="Đọc một câu mẫu bằng giọng đang chọn">
            Nghe thử
          </button>
          <span className="grow" />
          <button onClick={onClose} disabled={!!busy}>
            Đóng
          </button>
          <button className="primary" onClick={save} disabled={!!busy || !key.trim() || !region.trim()}>
            Lưu key
          </button>
        </div>
      </div>
    </div>
  );
}
