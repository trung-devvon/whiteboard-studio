// Bảng 5 màu "Giấy & Mực" — giữ khớp với :root trong App.css (skill .claude/skills/whiteboard-ui).
// Dùng cho chỗ không đọc được biến CSS (Konva, SVG trong trình chỉnh bàn tay).
export const PALETTE = {
  white: "#FFFFFF",
  beige: "#F3EDE4",
  sand: "#E2D8C9",
  ink: "#2B2621",
  clay: "#8E4A2F",
} as const;

/** Mực với độ trong — chỉ cho bóng/vùng phủ, không bao giờ cho chữ. */
export const inkAlpha = (a: number) => `rgba(43, 38, 33, ${a})`;
/** Đất nung với độ trong — chỉ cho vùng phủ, không bao giờ cho chữ. */
export const clayAlpha = (a: number) => `rgba(142, 74, 47, ${a})`;

/** Màu mặc định của NỘI DUNG dự án (chữ viết tay, nền cảnh) — không phải màu giao diện. */
export const CONTENT = {
  ink: "#1D1D1F", // palette-ignore
  background: "#FFFFFF", // palette-ignore
} as const;
