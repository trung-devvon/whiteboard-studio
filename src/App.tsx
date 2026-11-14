import SpikeLab from "./spike/SpikeLab";
import "./App.css";

// Giai đoạn 0: màn hình tạm chỉ chứa Spike Lab. Editor thật bắt đầu ở Giai đoạn 3.
export default function App() {
  return (
    <main className="container">
      <h1>Whiteboard Studio</h1>
      <p className="muted">Giai đoạn 0 — nền móng & kiểm chứng công nghệ</p>
      <SpikeLab />
    </main>
  );
}
