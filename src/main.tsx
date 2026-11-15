import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

async function start() {
  // chạy trong trình duyệt thường khi phát triển giao diện → giả lập lệnh Tauri
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    await (await import("./dev/mockIpc")).installMock();
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void start();
