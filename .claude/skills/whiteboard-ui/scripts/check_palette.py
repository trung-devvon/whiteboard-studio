"""Kiểm tra giao diện Whiteboard Studio theo bảng 5 màu.

Chạy:  python .claude/skills/whiteboard-ui/scripts/check_palette.py [thư-mục=src]

Báo lỗi khi:
  - có mã màu (#hex, rgb/rgba) không thuộc bảng 5 màu (rgba chỉ được dùng RGB của bảng màu);
  - chữ (`color:` / `fill` của Text) dùng màu có alpha < 1;
  - CSS có `opacity:` < 1 (làm mờ cả chữ bên trong).
In thêm bảng tương phản các cặp chữ/nền được phép. Mã thoát 1 nếu có lỗi.
"""

import pathlib
import re
import sys

PALETTE = {
    "white": "#FFFFFF",
    "beige": "#F3EDE4",
    "sand": "#E2D8C9",
    "ink": "#2B2621",
    "clay": "#8E4A2F",
}
# cặp chữ/nền được phép trong skill — mọi cặp phải >= 4.5:1
TEXT_PAIRS = [
    ("ink", "white"), ("ink", "beige"), ("ink", "sand"),
    ("clay", "white"), ("clay", "beige"), ("clay", "sand"),
    ("white", "ink"), ("white", "clay"),
]


def rgb(h):
    h = h.lstrip("#")
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def lum(c):
    f = lambda v: v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (f(v / 255) for v in c)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


ALLOWED = {rgb(v) for v in PALETTE.values()}
HEX = re.compile(r"#(?:[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b")
RGBA = re.compile(r"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)")
OPACITY = re.compile(r"(?<![-\w])opacity\s*:\s*([\d.]+)")
TEXT_ALPHA = re.compile(r"(?<![-\w])color\s*:\s*rgba\([^)]*,\s*(0?\.\d+)\s*\)")
IGNORE = "palette-ignore"  # dòng chứa màu của NỘI DUNG người dùng (mặc định màu mực…), không phải giao diện


def scan(root):
    errors = []
    for p in sorted(pathlib.Path(root).rglob("*")):
        if p.suffix not in {".css", ".tsx", ".ts"} or "bindings" in p.parts or "wasm" in p.parts:
            continue
        for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
            if IGNORE in line:
                continue
            where = f"{p}:{n}"
            for m in HEX.finditer(line):
                if rgb(m.group()) not in ALLOWED:
                    errors.append(f"{where}: màu ngoài bảng {m.group()}")
            for m in RGBA.finditer(line):
                if tuple(int(m.group(i)) for i in (1, 2, 3)) not in ALLOWED:
                    errors.append(f"{where}: màu ngoài bảng {m.group()}")
            for m in OPACITY.finditer(line):
                if float(m.group(1)) < 1:
                    errors.append(f"{where}: opacity {m.group(1)} làm mờ chữ — đổi nền/viền thay vì làm mờ")
            for m in TEXT_ALPHA.finditer(line):
                errors.append(f"{where}: chữ có alpha {m.group(1)} — chữ luôn đặc 100%")
    return errors


def main():
    print("Tương phản chữ/nền (cần >= 4.5):")
    bad = False
    for fg, bg in TEXT_PAIRS:
        c = contrast(rgb(PALETTE[fg]), rgb(PALETTE[bg]))
        bad |= c < 4.5
        print(f"  {fg:>5} trên {bg:<5} {c:5.2f}{'  << THIẾU' if c < 4.5 else ''}")
    errors = scan(sys.argv[1] if len(sys.argv) > 1 else "src")
    for e in errors:
        print(e)
    print(f"{len(errors)} lỗi." if errors else "Không có lỗi màu.")
    sys.exit(1 if errors or bad else 0)


if __name__ == "__main__":
    main()
