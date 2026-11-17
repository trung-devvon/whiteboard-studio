// Phát giọng đọc khi xem trước: giải mã WAV bằng Web Audio, phát từ một thời điểm, dừng khi tạm dừng/tua.

let ctx: AudioContext | null = null;
const audioCtx = () => (ctx ??= new AudioContext());

export class NarrationAudio {
  private source: AudioBufferSourceNode | null = null;

  private constructor(private buffer: AudioBuffer) {}

  static async load(wav: Uint8Array) {
    // decodeAudioData cần ArrayBuffer riêng (nó "lấy" luôn buffer)
    const copy = wav.slice().buffer;
    return new NarrationAudio(await audioCtx().decodeAudioData(copy));
  }

  /** Phát từ thời điểm `ms` (tính từ đầu video). */
  play(ms: number) {
    this.stop();
    const c = audioCtx();
    void c.resume();
    const at = ms / 1000;
    if (at >= this.buffer.duration) return;
    const s = c.createBufferSource();
    s.buffer = this.buffer;
    s.connect(c.destination);
    s.start(0, at);
    this.source = s;
  }

  stop() {
    try {
      this.source?.stop();
    } catch {
      // đã dừng
    }
    this.source?.disconnect();
    this.source = null;
  }
}

/** Phát một đoạn WAV ngắn (nghe thử giọng). */
export async function playOnce(wav: ArrayBuffer) {
  const c = audioCtx();
  await c.resume();
  const s = c.createBufferSource();
  s.buffer = await c.decodeAudioData(wav.slice(0));
  s.connect(c.destination);
  s.start();
}
