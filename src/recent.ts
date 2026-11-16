// Danh sách dự án mở gần đây — chỉ là tiện ích trên máy này (localStorage của WebView).
const KEY = "wb.recent-projects";
const MAX = 8;

export type Recent = { root: string; openedAt: number };

export function recentProjects(): Recent[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((r) => typeof r?.root === "string") : [];
  } catch {
    return [];
  }
}

function store(list: Recent[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list.slice(0, MAX)));
  } catch {
    /* không lưu được thì thôi */
  }
}

export function rememberProject(root: string) {
  store([{ root, openedAt: Date.now() }, ...recentProjects().filter((r) => r.root !== root)]);
}

export function forgetProject(root: string) {
  store(recentProjects().filter((r) => r.root !== root));
}
