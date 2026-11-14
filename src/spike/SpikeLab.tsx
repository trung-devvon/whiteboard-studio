// TẠM — Giai đoạn 0. Trang đo SPIKE S2 (WASM preview) + S5 (IPC nhị phân). Xoá ở Giai đoạn 3.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import init, { Player, syntheticPlanBytes } from "../../spikes/s2-wasm-preview/pkg/s2_wasm_preview.js";

const W = 960;
const H = 540;
const isTauri = "__TAURI_INTERNALS__" in window;

type IpcRow = { strokes: number; bytes: number; binaryMs: number; jsonMs: number; jsonChars: number; wasmLoadMs: number };
type PlayRow = { strokes: number; avgMs: number; p95Ms: number; maxMs: number; rafFps: number; seekMs: number };

const median = (a: number[]) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
const pct = (a: number[], p: number) => [...a].sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(a.length * p))];
const r1 = (n: number) => Math.round(n * 10) / 10;

async function measureIpc(strokes: number): Promise<IpcRow> {
  const bin: number[] = [];
  const js: number[] = [];
  let buf = new ArrayBuffer(0);
  let jsonChars = 0;
  for (let k = 0; k < 3; k++) {
    let t = performance.now();
    buf = await invoke<ArrayBuffer>("spike_plan_binary", { strokes });
    bin.push(performance.now() - t);
    t = performance.now();
    const plan = await invoke<unknown>("spike_plan_json", { strokes });
    js.push(performance.now() - t);
    if (k === 0) jsonChars = JSON.stringify(plan).length;
  }
  const t = performance.now();
  const player = new Player(W, H, new Uint8Array(buf));
  const wasmLoadMs = performance.now() - t;
  player.free();
  return { strokes, bytes: buf.byteLength, binaryMs: r1(median(bin)), jsonMs: r1(median(js)), jsonChars, wasmLoadMs: r1(wasmLoadMs) };
}

async function measurePlayback(
  memory: WebAssembly.Memory,
  canvas: HTMLCanvasElement,
  strokes: number,
  bytes: Uint8Array,
): Promise<PlayRow> {
  const ctx = canvas.getContext("2d")!;
  const player = new Player(W, H, bytes);
  const len = player.frameLen();
  const draw = (progress: number) => {
    const ptr = player.render(progress);
    ctx.putImageData(new ImageData(new Uint8ClampedArray(memory.buffer, ptr, len), W, H), 0, 0);
  };
  // 1) Đo chi phí mỗi khung (render WASM + đẩy lên canvas), 300 khung = 10s video ở 30fps
  const times: number[] = [];
  const N = 300;
  for (let i = 0; i < N; i++) {
    const t = performance.now();
    draw((i + 1) / N);
    times.push(performance.now() - t);
  }
  // 2) Tua ngược về giữa (phải vẽ lại từ đầu)
  let t = performance.now();
  draw(0.5);
  const seekMs = performance.now() - t;
  // 3) Phát thật bằng requestAnimationFrame trong 3 giây
  draw(0);
  let frames = 0;
  const start = performance.now();
  await new Promise<void>((done) => {
    const tick = (now: number) => {
      const p = (now - start) / 3000;
      draw(Math.min(1, p));
      frames++;
      if (p < 1) requestAnimationFrame(tick);
      else done();
    };
    requestAnimationFrame(tick);
  });
  const rafFps = frames / ((performance.now() - start) / 1000);
  player.free();
  const avg = times.reduce((a, b) => a + b, 0) / times.length;
  return { strokes, avgMs: r1(avg), p95Ms: r1(pct(times, 0.95)), maxMs: r1(Math.max(...times)), rafFps: r1(rafFps), seekMs: r1(seekMs) };
}

export default function SpikeLab() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [log, setLog] = useState<string[]>([]);
  const [running, setRunning] = useState(false);
  const started = useRef(false);
  const say = (s: string) => setLog((l) => [...l, s]);

  async function runAll() {
    setRunning(true);
    setLog([]);
    try {
      const wasm = await init();
      say(`Môi trường: ${isTauri ? "Tauri WebView" : "trình duyệt"} · ${navigator.userAgent}`);
      const ipc: IpcRow[] = [];
      if (isTauri) {
        for (const n of [2000, 20000, 80000]) {
          const row = await measureIpc(n);
          ipc.push(row);
          say(`S5 · ${n} nét · ${(row.bytes / 1e6).toFixed(2)}MB nhị phân: ${row.binaryMs}ms | JSON (${(row.jsonChars / 1e6).toFixed(1)}MB): ${row.jsonMs}ms | nạp WASM: ${row.wasmLoadMs}ms`);
        }
      } else {
        say("S5 bỏ qua (cần chạy trong app Tauri)");
      }
      const play: PlayRow[] = [];
      for (const n of [2000, 10000]) {
        const bytes = isTauri
          ? new Uint8Array(await invoke<ArrayBuffer>("spike_plan_binary", { strokes: n }))
          : syntheticPlanBytes(n, 42, W, H);
        const row = await measurePlayback(wasm.memory, canvas.current!, n, bytes);
        play.push(row);
        say(`S2 · ${n} nét @${W}×${H}: TB ${row.avgMs}ms/khung · p95 ${row.p95Ms}ms · max ${row.maxMs}ms · rAF ${row.rafFps}fps · tua ngược ${row.seekMs}ms`);
      }
      const report = { at: new Date().toISOString(), userAgent: navigator.userAgent, isTauri, resolution: `${W}x${H}`, ipc, play };
      (window as unknown as { __spikeReport: unknown }).__spikeReport = report;
      if (isTauri) say(`Đã ghi: ${await invoke<string>("spike_report", { report })}`);
      say("XONG");
    } catch (e) {
      say(`LỖI: ${e}`);
    } finally {
      setRunning(false);
    }
  }

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    if (isTauri)
      invoke<boolean>("spike_autorun").then((auto) => {
        if (auto) void runAll();
      });
  }, []);

  return (
    <section className="spike">
      <h2>Spike Lab — Giai đoạn 0 (S2 WASM preview · S5 IPC nhị phân)</h2>
      <button disabled={running} onClick={runAll}>{running ? "Đang đo…" : "Chạy đo"}</button>
      <canvas ref={canvas} width={W} height={H} />
      <pre>{log.join("\n")}</pre>
    </section>
  );
}
