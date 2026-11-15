// Thanh tua: phát/dừng, kéo tới thời điểm bất kỳ; vạch chia các cảnh.
import type { Preview } from "../player";

type Props = {
  preview: Preview | null;
  time: number;
  playing: boolean;
  onPlay: () => void;
  onSeek: (t: number) => void;
};

const fmt = (ms: number) => {
  const s = Math.max(0, ms) / 1000;
  return `${Math.floor(s / 60)}:${(s % 60).toFixed(1).padStart(4, "0")}`;
};

export default function Timebar({ preview, time, playing, onPlay, onSeek }: Props) {
  const total = preview?.duration ?? 0;
  return (
    <div className="timebar">
      <button className="play" onClick={onPlay} disabled={!preview} title="Phát / dừng (Space)">
        {playing ? "❚❚" : "▶"}
      </button>
      <span className="time">
        {fmt(time)} / {fmt(total)}
      </span>
      <div className="track">
        {preview?.sceneStarts.slice(1).map((s, i) => (
          <i key={i} className="scene-mark" style={{ left: `${(s / total) * 100}%` }} />
        ))}
        <input
          type="range"
          min={0}
          max={Math.max(1, total - 1)}
          step={10}
          value={Math.min(time, Math.max(0, total - 1))}
          disabled={!preview}
          onChange={(e) => onSeek(Number(e.target.value))}
        />
      </div>
    </div>
  );
}
