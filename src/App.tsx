// Editor Whiteboard Studio (Giai đoạn 3): mở/lưu dự án, sắp xếp cảnh & element, xem trước bằng Player WASM,
// xuất MP4. Logic nặng (phân tích ảnh, render, xuất) nằm ở Rust; ở đây chỉ là giao diện.
import { useCallback, useEffect, useRef, useState } from "react";
import { ask, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore } from "zustand";
import { api, errorText, type ElementInfo, type HandEntry } from "./api";
import { Preview } from "./player";
import { forgetProject, recentProjects, rememberProject } from "./recent";
import { clearHistory, redo, undo, useEditor } from "./store";
import Stage from "./components/Stage";
import ScenePanel from "./components/ScenePanel";
import Inspector from "./components/Inspector";
import Timebar from "./components/Timebar";
import ExportDialog from "./components/ExportDialog";
import HandEditor from "./components/HandEditor";
import IconPicker from "./components/IconPicker";
import { CONTENT } from "./theme";
import "./App.css";

/** Độ phân giải Player trong editor (khung xuất thật dùng cỡ dự án). */
const PREVIEW_W = 960;
const AUTOSAVE_MS = 4000;

export default function App() {
  const root = useEditor((s) => s.root);
  return root ? <Editor /> : <Welcome />;
}

async function openProject(path: string) {
  const o = await api.open(path);
  let project = o.project;
  if (o.recovery) {
    if (confirm("Lần trước app đã tắt khi còn thay đổi chưa lưu. Khôi phục các thay đổi đó?")) project = o.recovery;
    else await api.discardAutosave(o.root);
  }
  useEditor.getState().load(o.root, project, o.project);
  clearHistory();
  rememberProject(o.root);
}

/** Tạo dự án: chọn nơi lưu thư mục `.wbproj` (trống hoặc điền sẵn dự án mẫu). */
async function newProject(sample: boolean) {
  const path = await saveDialog({
    defaultPath: sample ? "du-an-mau.wbproj" : "bai-hoc.wbproj",
    title: "Chọn nơi lưu dự án (app sẽ tạo một thư mục .wbproj)",
  });
  if (!path) return;
  const o = await api.create(path, sample);
  useEditor.getState().load(o.root, o.project, o.project);
  clearHistory();
  rememberProject(o.root);
}

async function pickAndOpen() {
  const dir = await open({ directory: true, title: "Mở dự án — chọn thư mục .wbproj" });
  if (typeof dir === "string") await openProject(dir);
}

function Welcome() {
  const [err, setErr] = useState<string | null>(null);
  const [recent, setRecent] = useState(recentProjects);
  const run = (f: () => Promise<void>) => () => {
    setErr(null);
    f().catch((e) => setErr(errorText(e)));
  };
  const openRecent = (root: string) =>
    openProject(root).catch((e) => {
      setErr(`Không mở được ${root}: ${errorText(e)}`);
      forgetProject(root);
      setRecent(recentProjects());
    });
  const name = (root: string) => root.split(/[\\/]/).pop()?.replace(/\.wbproj$/i, "");

  return (
    <main className="welcome">
      <h1>Whiteboard Studio</h1>
      <p className="muted">Tạo video "bàn tay vẽ trên bảng trắng" từ ảnh và chữ.</p>

      <div className="welcome-cards">
        <button className="card primary" onClick={run(() => newProject(true))}>
          <strong>Thử dự án mẫu</strong>
          <span>Tạo sẵn 2 cảnh có chữ và hình — bấm ▶ để xem ngay.</span>
        </button>
        <button className="card" onClick={run(() => newProject(false))}>
          <strong>Dự án mới</strong>
          <span>Bắt đầu với một cảnh trống.</span>
        </button>
        <button className="card" onClick={run(pickAndOpen)}>
          <strong>Mở dự án…</strong>
          <span>Chọn một thư mục .wbproj đã có.</span>
        </button>
      </div>

      {recent.length > 0 && (
        <section className="recent">
          <h3>Mở gần đây</h3>
          <ul>
            {recent.map((r) => (
              <li key={r.root}>
                <button className="link" onClick={() => openRecent(r.root)} title={r.root}>
                  {name(r.root)}
                </button>
                <small>{r.root}</small>
              </li>
            ))}
          </ul>
        </section>
      )}

      <details className="explain">
        <summary>Dự án là gì?</summary>
        <p>
          Mỗi video là một <b>dự án</b> — một thư mục <code>tên.wbproj</code> trên máy bạn, chứa file <code>project.json</code>{" "}
          (các cảnh, vị trí, chữ, cài đặt) và thư mục <code>assets/</code> (bản sao các ảnh bạn thêm vào). Nhờ vậy dự án mang đi
          máy khác vẫn đủ ảnh. App tự lưu bản phòng sự cố khi bạn chưa bấm Lưu.
        </p>
        <p>
          Cách làm video: thêm <b>cảnh</b> (cột trái) → thêm <b>ảnh/chữ</b> (thanh trên) → kéo vào chỗ trên bảng → thứ tự trong
          danh sách "Thứ tự vẽ" là thứ tự bàn tay vẽ → bấm ▶ để xem → <b>Xuất video</b>.
        </p>
      </details>
      {err && <p className="err">{err}</p>}
    </main>
  );
}

function Editor() {
  const root = useEditor((s) => s.root)!;
  const project = useEditor((s) => s.project)!;
  const saved = useEditor((s) => s.saved);
  const sceneIndex = useEditor((s) => s.sceneIndex);
  const selectedId = useEditor((s) => s.selectedId);
  const canUndo = useStore(useEditor.temporal, (s) => s.pastStates.length > 0);
  const canRedo = useStore(useEditor.temporal, (s) => s.futureStates.length > 0);
  const dirty = project !== saved;

  const [preview, setPreview] = useState<Preview | null>(null);
  const [info, setInfo] = useState<ElementInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hands, setHands] = useState<HandEntry[]>([]);
  const [exporting, setExporting] = useState(false);
  const [handEditor, setHandEditor] = useState(false);
  const [iconPicker, setIconPicker] = useState(false);
  /** Tăng khi bộ tay đổi trên đĩa → biên dịch lại xem trước dù dự án không đổi. */
  const [handVersion, setHandVersion] = useState(0);
  const [message, setMessage] = useState<string | null>(null);

  // thời gian: khi đang sửa, ghim ở cuối cảnh đang chọn (thấy đủ mọi element để sắp xếp)
  const [time, setTime] = useState(0);
  const [pinEnd, setPinEnd] = useState(true);
  const [playing, setPlaying] = useState(false);
  const shownTime = pinEnd && preview ? Math.max(0, (preview.sceneEnds[sceneIndex] ?? preview.duration) - 1) : time;

  const loadHands = useCallback(() => api.hands().then(setHands).catch(() => {}), []);
  useEffect(() => {
    void loadHands();
  }, [loadHands]);

  // biên dịch lại (có đệm phân tích) mỗi khi dự án đổi — gom các thay đổi liên tiếp
  const req = useRef(0);
  useEffect(() => {
    const id = ++req.current;
    const timer = setTimeout(async () => {
      setBusy(true);
      try {
        const { header, bundle } = await api.compilePreview(root, project, PREVIEW_W);
        const p = await Preview.load(bundle, PREVIEW_W);
        if (id !== req.current) return p.free();
        setPreview(p);
        setInfo(header.elements);
        setError(null);
      } catch (e) {
        if (id === req.current) setError(errorText(e));
      } finally {
        if (id === req.current) setBusy(false);
      }
    }, 350);
    return () => clearTimeout(timer);
  }, [root, project, handVersion]);

  // Player cũ được giải phóng khi có Player mới (bộ nhớ WASM không tự thu gom)
  useEffect(() => () => preview?.free(), [preview]);

  // phát
  useEffect(() => {
    if (!playing || !preview) return;
    let raf = 0;
    let last = performance.now();
    let t = pinEnd ? (preview.sceneStarts[sceneIndex] ?? 0) : time;
    if (t >= preview.duration - 1) t = 0;
    setPinEnd(false);
    const tick = (now: number) => {
      t += now - last;
      last = now;
      if (t >= preview.duration) {
        setTime(preview.duration - 1);
        setPlaying(false);
        return;
      }
      setTime(t);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
    // chỉ chạy lại khi bật/tắt phát hoặc Player mới
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playing, preview]);

  // tua → đổi cảnh theo thời điểm
  const seek = (t: number) => {
    setPlaying(false);
    setPinEnd(false);
    setTime(t);
    const si = preview?.sceneAt(t) ?? 0;
    if (si !== useEditor.getState().sceneIndex) useEditor.getState().selectScene(si);
  };

  // chọn cảnh / chọn element → về chế độ sửa
  useEffect(() => {
    if (!playing) setPinEnd(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sceneIndex, selectedId]);

  const save = useCallback(async () => {
    const { root, project } = useEditor.getState();
    if (!root || !project) return;
    try {
      await api.save(root, project);
      useEditor.getState().markSaved();
      setMessage("Đã lưu");
      setTimeout(() => setMessage(null), 1500);
    } catch (e) {
      setError(`Lưu thất bại: ${errorText(e)}`);
      throw e;
    }
  }, []);

  // tự lưu bản phòng sự cố khi có thay đổi chưa lưu
  useEffect(() => {
    if (!dirty) return;
    const t = setTimeout(() => api.autosave(root, project).catch(() => {}), AUTOSAVE_MS);
    return () => clearTimeout(t);
  }, [dirty, root, project]);

  // đóng cửa sổ khi còn thay đổi chưa lưu → hỏi
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  useEffect(() => {
    let win: ReturnType<typeof getCurrentWindow>;
    try {
      win = getCurrentWindow();
    } catch {
      return; // không chạy trong Tauri (chế độ thử giao diện trên trình duyệt)
    }
    const un = win.onCloseRequested(async (e) => {
      if (!dirtyRef.current) return;
      e.preventDefault();
      const keep = await ask("Dự án còn thay đổi chưa lưu. Lưu trước khi đóng?", {
        title: "Whiteboard Studio",
        kind: "warning",
        okLabel: "Lưu rồi đóng",
        cancelLabel: "Đóng, không lưu",
      });
      try {
        if (keep) await save();
        else await api.discardAutosave(useEditor.getState().root!);
      } catch {
        return; // lưu lỗi → giữ cửa sổ mở
      }
      await win.destroy();
    });
    return () => {
      void un.then((f) => f());
    };
  }, [save]);

  async function addImage() {
    const file = await open({
      title: "Chọn ảnh",
      filters: [{ name: "Ảnh", extensions: ["png", "jpg", "jpeg", "webp", "svg"] }],
    });
    if (typeof file !== "string") return;
    try {
      const src = await api.importAsset(root, file);
      useEditor.getState().addElement({ type: "image", src }, { x: 460, y: 190, w: 1000, h: 700 });
    } catch (e) {
      setError(errorText(e));
    }
  }

  function addText() {
    useEditor
      .getState()
      .addElement(
        { type: "text", text: "Chữ viết tay", size: 72, color: CONTENT.ink, align: "center", line_height: 1.35 },
        { x: 360, y: 120, w: 1200, h: 110 },
      );
  }

  const leave = (f: () => Promise<void>) => () => {
    if (dirty && !confirm("Bỏ các thay đổi chưa lưu?")) return;
    f().catch((e) => setError(errorText(e)));
  };

  // phím tắt
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement)?.closest("input, textarea, select");
      const k = e.key.toLowerCase();
      if (e.ctrlKey && k === "s") {
        e.preventDefault();
        void save().catch(() => {});
      } else if (typing || document.querySelector(".modal-back")) {
        return;
      } else if (e.ctrlKey && k === "z" && !e.shiftKey) {
        e.preventDefault();
        undo();
      } else if (e.ctrlKey && (k === "y" || (k === "z" && e.shiftKey))) {
        e.preventDefault();
        redo();
      } else if ((k === "delete" || k === "backspace") && useEditor.getState().selectedId) {
        useEditor.getState().deleteElement(useEditor.getState().selectedId!);
      } else if (k === " ") {
        e.preventDefault();
        setPlaying((p) => !p);
      } else if (k === "escape") {
        useEditor.getState().select(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save]);

  const name = root.split(/[\\/]/).pop();
  return (
    <div className="editor">
      <header className="toolbar">
        <strong className="title" title={root}>
          {name}
          {dirty && <span className="dirty" title="Có thay đổi chưa lưu"> •</span>}
        </strong>
        <button onClick={leave(async () => useEditor.getState().close())} title="Về màn hình chào">
          ⌂
        </button>
        <button onClick={leave(() => newProject(false))}>Mới</button>
        <button onClick={leave(pickAndOpen)}>Mở</button>
        <button onClick={() => void save().catch(() => {})} disabled={!dirty} title="Ctrl+S">
          Lưu
        </button>
        <span className="sep" />
        <button onClick={undo} disabled={!canUndo} title="Hoàn tác (Ctrl+Z)">
          ↶
        </button>
        <button onClick={redo} disabled={!canRedo} title="Làm lại (Ctrl+Y)">
          ↷
        </button>
        <span className="sep" />
        <button onClick={addImage}>＋ Ảnh</button>
        <button onClick={addText}>＋ Chữ</button>
        <button onClick={() => setIconPicker(true)} title="Icon, mũi tên có sẵn">
          ＋ Icon
        </button>
        <span className="grow" />
        {message && <span className="msg">{message}</span>}
        <button className="primary" onClick={() => setExporting(true)}>
          Xuất video
        </button>
      </header>

      <ScenePanel sceneTimes={preview ? preview.sceneEnds.map((e, i) => e - preview.sceneStarts[i]) : []} />
      <main className="center">
        <Stage preview={preview} time={shownTime} playing={playing} busy={busy} error={error} />
        <Timebar preview={preview} time={shownTime} playing={playing} onPlay={() => setPlaying((p) => !p)} onSeek={seek} />
      </main>
      <Inspector info={info} hands={hands} onManageHands={() => setHandEditor(true)} />

      {exporting && <ExportDialog root={root} project={project} onClose={() => setExporting(false)} />}
      {iconPicker && (
        <IconPicker
          onClose={() => setIconPicker(false)}
          onPick={(icon) => {
            useEditor.getState().addElement({ type: "image", src: icon.src }, { x: 760, y: 340, w: 400, h: 400 });
            setIconPicker(false);
          }}
        />
      )}
      {handEditor && (
        <HandEditor
          current={project.settings?.hand ?? null}
          onUse={(hand) => {
            useEditor.getState().updateSettings({ hand });
            void loadHands();
          }}
          onChanged={() => {
            void loadHands();
            setHandVersion((v) => v + 1);
          }}
          onClose={() => {
            setHandEditor(false);
            void loadHands();
          }}
        />
      )}
    </div>
  );
}
