import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. only the web UI (src/) needs live reload. Watching Rust build output (target/, tens of
      //    thousands of files), crates, assets and docs made dev-server start and page loads very slow.
      ignored: [
        "**/src-tauri/**",
        "**/target/**",
        "**/crates/**",
        "**/assets/**",
        "**/docs/**",
        "**/tests/**",
        "**/not_push_git/**",
        "**/.claude/**",
      ],
    },
  },
}));
