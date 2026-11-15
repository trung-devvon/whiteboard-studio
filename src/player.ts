// Player WASM (crates/wb-player): dựng Engine từ gói biên dịch, vẽ khung thẳng từ bộ nhớ WASM lên canvas.
// Gói WASM sinh bởi `npm run build:wasm` vào src/wasm/pkg (không commit).
import init, { Player } from "./wasm/pkg/wb_player.js";

let memory: WebAssembly.Memory | null = null;

async function ensureWasm() {
  if (!memory) memory = (await init()).memory;
  return memory;
}

export class Preview {
  private constructor(
    private player: Player,
    private mem: WebAssembly.Memory,
    readonly width: number,
    readonly height: number,
    readonly duration: number,
    readonly sceneStarts: number[],
    readonly sceneEnds: number[],
  ) {}

  static async load(bundle: Uint8Array, width: number) {
    const mem = await ensureWasm();
    const p = new Player(bundle, width);
    return new Preview(
      p,
      mem,
      p.width(),
      p.height(),
      p.durationMs(),
      Array.from(p.sceneStarts()),
      Array.from(p.sceneEnds()),
    );
  }

  private freed = false;

  /** Vẽ khung tại `t` (ms) lên canvas (canvas phải có đúng cỡ width×height). */
  draw(ctx: CanvasRenderingContext2D, t: number) {
    if (this.freed) return;
    const ptr = this.player.render(t);
    const len = this.player.frameLen();
    // bộ nhớ WASM có thể lớn lên sau render → lấy buffer mới mỗi lần
    const px = new Uint8ClampedArray(this.mem.buffer, ptr, len);
    ctx.putImageData(new ImageData(px, this.width, this.height), 0, 0);
  }

  /** Scene chứa thời điểm `t`. */
  sceneAt(t: number) {
    const i = this.sceneEnds.findIndex((e) => t < e);
    return i < 0 ? this.sceneEnds.length - 1 : i;
  }

  /** Giải phóng bộ nhớ WASM; gọi nhiều lần không sao. */
  free() {
    if (this.freed) return;
    this.freed = true;
    this.player.free();
  }
}
