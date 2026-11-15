// Trạng thái editor (zustand). Lịch sử undo/redo (zundo) chỉ theo dõi tài liệu `project`;
// mỗi thao tác hoàn chỉnh (thả chuột, bấm Enter) là một bước.
import { create } from "zustand";
import { temporal } from "zundo";
import type { Element, ElementKind, Project, ProjectSettings, Scene, Transform } from "./bindings/model";

export const REF_W = 1920;
export const REF_H = 1080;

type EditorState = {
  root: string | null;
  project: Project | null;
  /** Bản đã lưu gần nhất — so sánh để biết còn thay đổi chưa lưu. */
  saved: Project | null;
  sceneIndex: number;
  selectedId: string | null;

  load: (root: string, project: Project, saved: Project) => void;
  close: () => void;
  markSaved: () => void;
  selectScene: (i: number) => void;
  select: (id: string | null) => void;

  addScene: () => void;
  deleteScene: (i: number) => void;
  renameScene: (i: number, name: string) => void;
  moveScene: (i: number, dir: -1 | 1) => void;
  updateScene: (i: number, patch: Partial<Scene>) => void;
  updateSettings: (patch: Partial<ProjectSettings>) => void;

  addElement: (kind: ElementKind, transform: Transform) => void;
  updateElement: (id: string, patch: Partial<Omit<Element, "id">>) => void;
  updateKind: (id: string, patch: Partial<ElementKind>) => void;
  deleteElement: (id: string) => void;
  moveElement: (id: string, dir: -1 | 1) => void;
};

let seq = 0;
export const newId = (prefix: string) => `${prefix}${Date.now().toString(36)}${(seq++).toString(36)}`;

/** Sửa scene đang chọn (bất biến). */
function withScene(p: Project, i: number, f: (s: Scene) => Scene): Project {
  return { ...p, scenes: p.scenes.map((s, k) => (k === i ? f(s) : s)) };
}

function withElement(p: Project, id: string, f: (e: Element) => Element): Project {
  return {
    ...p,
    scenes: p.scenes.map((s) =>
      s.elements.some((e) => e.id === id) ? { ...s, elements: s.elements.map((e) => (e.id === id ? f(e) : e)) } : s,
    ),
  };
}

export const useEditor = create<EditorState>()(
  temporal(
    (set, get) => {
      const edit = (f: (p: Project) => Project) => {
        const p = get().project;
        if (p) set({ project: f(p) });
      };
      return {
        root: null,
        project: null,
        saved: null,
        sceneIndex: 0,
        selectedId: null,

        load: (root, project, saved) => set({ root, project, saved, sceneIndex: 0, selectedId: null }),
        close: () => set({ root: null, project: null, saved: null, sceneIndex: 0, selectedId: null }),
        markSaved: () => set({ saved: get().project }),
        selectScene: (i) => set({ sceneIndex: i, selectedId: null }),
        select: (id) => set({ selectedId: id }),

        addScene: () => {
          const p = get().project;
          if (!p) return;
          const n = p.scenes.length + 1;
          const scene: Scene = { id: newId("s"), name: `Cảnh ${n}`, background: null, elements: [], tail_ms: 1500 };
          set({ project: { ...p, scenes: [...p.scenes, scene] }, sceneIndex: p.scenes.length, selectedId: null });
        },
        deleteScene: (i) => {
          const p = get().project;
          if (!p || p.scenes.length <= 1) return;
          const scenes = p.scenes.filter((_, k) => k !== i);
          set({ project: { ...p, scenes }, sceneIndex: Math.min(get().sceneIndex, scenes.length - 1), selectedId: null });
        },
        renameScene: (i, name) => edit((p) => withScene(p, i, (s) => ({ ...s, name }))),
        moveScene: (i, dir) => {
          const p = get().project;
          const j = i + dir;
          if (!p || j < 0 || j >= p.scenes.length) return;
          const scenes = [...p.scenes];
          [scenes[i], scenes[j]] = [scenes[j], scenes[i]];
          set({ project: { ...p, scenes }, sceneIndex: j });
        },
        updateScene: (i, patch) => edit((p) => withScene(p, i, (s) => ({ ...s, ...patch }))),
        updateSettings: (patch) => edit((p) => ({ ...p, settings: { ...p.settings, ...patch } })),

        addElement: (kind, transform) => {
          const id = newId("e");
          edit((p) =>
            withScene(p, get().sceneIndex, (s) => ({ ...s, elements: [...s.elements, { id, kind, transform }] })),
          );
          set({ selectedId: id });
        },
        updateElement: (id, patch) => edit((p) => withElement(p, id, (e) => ({ ...e, ...patch }))),
        updateKind: (id, patch) =>
          edit((p) => withElement(p, id, (e) => ({ ...e, kind: { ...e.kind, ...patch } as ElementKind }))),
        deleteElement: (id) => {
          edit((p) => ({ ...p, scenes: p.scenes.map((s) => ({ ...s, elements: s.elements.filter((e) => e.id !== id) })) }));
          if (get().selectedId === id) set({ selectedId: null });
        },
        moveElement: (id, dir) =>
          edit((p) =>
            withScene(p, get().sceneIndex, (s) => {
              const i = s.elements.findIndex((e) => e.id === id);
              const j = i + dir;
              if (i < 0 || j < 0 || j >= s.elements.length) return s;
              const elements = [...s.elements];
              [elements[i], elements[j]] = [elements[j], elements[i]];
              return { ...s, elements };
            }),
          ),
      };
    },
    {
      // chỉ tài liệu dự án vào lịch sử; chọn scene/element không tạo bước undo
      partialize: (s) => ({ project: s.project }),
      equality: (a, b) => a.project === b.project,
      limit: 200,
    },
  ),
);

export const undo = () => useEditor.temporal.getState().undo();
export const redo = () => useEditor.temporal.getState().redo();
export const clearHistory = () => useEditor.temporal.getState().clear();

export function currentScene(s: { project: Project | null; sceneIndex: number }): Scene | null {
  return s.project?.scenes[s.sceneIndex] ?? null;
}

export function findElement(p: Project | null, id: string | null): Element | null {
  if (!p || !id) return null;
  for (const s of p.scenes) for (const e of s.elements) if (e.id === id) return e;
  return null;
}

/** Ô bao của element (px khung tham chiếu). */
export function boxOf(e: Element) {
  const t = e.transform;
  return { x: t.x ?? 0, y: t.y ?? 0, w: t.w ?? 0, h: t.h ?? 0 };
}
