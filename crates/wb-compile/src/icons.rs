//! Thư viện icon có sẵn (Lucide, ISC — `assets/icons/lucide/`): tìm theo tên/thẻ tiếng Anh, và theo
//! **từ khoá tiếng Việt** (gõ có dấu hay không dấu đều được) nhờ bảng dịch nhỏ bên dưới.
//!
//! Icon được thêm vào dự án như ảnh SVG thường (`builtin:icons/lucide/icons/<tên>.svg`) → đi theo path (Nhóm E).

use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Thư mục bộ icon trong `assets/`.
pub const LUCIDE_DIR: &str = "icons/lucide";

/// Từ khoá tiếng Việt (không dấu) → từ tiếng Anh dùng để tìm. Cụm dài được so trước.
const VI: &[(&str, &str)] = &[
    ("mui ten", "arrow"),
    ("bong den", "lightbulb"),
    ("y tuong", "lightbulb idea"),
    ("dien thoai", "phone smartphone"),
    ("may tinh", "laptop computer monitor"),
    ("dong ho", "clock watch"),
    ("thoi gian", "clock time timer hourglass"),
    ("lich", "calendar"),
    ("tien", "dollar coins money banknote wallet"),
    ("kinh doanh", "briefcase chart trending"),
    ("bieu do", "chart"),
    ("tang truong", "trending-up chart"),
    ("giam", "trending-down"),
    ("nguoi", "user person"),
    ("nhom", "users group"),
    ("gia dinh", "users"),
    ("nha", "house home"),
    ("truong hoc", "school graduation"),
    ("hoc", "book graduation school"),
    ("sach", "book"),
    ("but", "pen pencil"),
    ("thu", "mail envelope"),
    ("tin nhan", "message chat"),
    ("tro chuyen", "message chat"),
    ("cau hoi", "help question"),
    ("dau hoi", "help question"),
    ("canh bao", "alert warning triangle"),
    ("dung", "check"),
    ("sai", "x"),
    ("tim", "heart"),
    ("yeu", "heart"),
    ("sao", "star"),
    ("mat troi", "sun"),
    ("mat trang", "moon"),
    ("may", "cloud"),
    ("mua", "cloud-rain umbrella"),
    ("cay", "tree leaf sprout"),
    ("la", "leaf"),
    ("hoa", "flower"),
    ("nuoc", "droplet waves"),
    ("lua", "flame"),
    ("xe", "car bus truck bike"),
    ("may bay", "plane"),
    ("tau", "ship train"),
    ("ban do", "map"),
    ("vi tri", "map-pin location"),
    ("trai dat", "globe earth"),
    ("the gioi", "globe"),
    ("khoa", "lock key"),
    ("an toan", "shield lock"),
    ("cai dat", "settings cog"),
    ("banh rang", "cog settings"),
    ("cong cu", "wrench hammer tool"),
    ("tim kiem", "search"),
    ("kinh lup", "search zoom"),
    ("muc tieu", "target goal"),
    ("cup", "trophy award"),
    ("giai thuong", "trophy award medal"),
    ("qua", "gift"),
    ("gio hang", "shopping-cart"),
    ("mua sam", "shopping"),
    ("cua hang", "store"),
    ("suc khoe", "heart-pulse stethoscope"),
    ("benh vien", "hospital"),
    ("thuoc", "pill"),
    ("an", "utensils"),
    ("ca phe", "coffee"),
    ("nhac", "music"),
    ("anh", "image camera"),
    ("video", "video film"),
    ("loa", "volume speaker megaphone"),
    ("micro", "mic"),
    ("tai lieu", "file document"),
    ("thu muc", "folder"),
    ("du lieu", "database"),
    ("mang", "network wifi"),
    ("tai xuong", "download"),
    ("tai len", "upload"),
    ("chia se", "share"),
    ("lien ket", "link"),
    ("tang", "plus"),
    ("tru", "minus"),
    ("bat dau", "play rocket flag"),
    ("ten lua", "rocket"),
    ("co", "flag"),
    ("cau thang", "stairs"),
    ("buoc", "footprints stairs"),
    ("ket noi", "link plug"),
    ("pin", "battery"),
    ("dien", "zap plug"),
    ("phai", "right"),
    ("trai", "left"),
    ("len", "up"),
    ("tren", "up"),
    ("xuong", "down"),
    ("duoi", "down"),
    ("cong", "curve corner"),
    ("tron", "circle"),
    ("vuong", "square"),
    ("tam giac", "triangle"),
];

/// Bỏ dấu tiếng Việt, chữ thường.
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.to_lowercase()
        .replace('đ', "d")
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct IconHit {
    pub name: String,
    /// Giá trị ghi vào element ảnh, vd. `builtin:icons/lucide/icons/house.svg`.
    pub src: String,
}

pub struct IconIndex {
    /// (tên, các thẻ)
    entries: Vec<(String, Vec<String>)>,
}

/// Icon gợi ý khi chưa gõ gì: mũi tên và vài icon hay dùng trong video giải thích.
const DEFAULT: &[&str] = &[
    "arrow-right",
    "arrow-left",
    "arrow-up",
    "arrow-down",
    "move-right",
    "redo",
    "undo",
    "corner-down-right",
    "arrow-big-right",
    "trending-up",
    "lightbulb",
    "user",
    "users",
    "house",
    "book-open",
    "graduation-cap",
    "clock",
    "calendar",
    "dollar-sign",
    "chart-column",
    "target",
    "rocket",
    "heart",
    "star",
    "check",
    "x",
    "circle-help",
    "triangle-alert",
    "message-circle",
    "mail",
    "phone",
    "laptop",
    "globe",
    "map-pin",
    "settings",
    "search",
];

impl IconIndex {
    /// Đọc `tags.json` trong thư mục bộ icon (`assets/icons/lucide`).
    pub fn load(dir: &Path) -> Result<Self> {
        let p = dir.join("tags.json");
        let text = std::fs::read_to_string(&p).with_context(|| format!("không đọc được {}", p.display()))?;
        let map: BTreeMap<String, Vec<String>> = serde_json::from_str(&text)?;
        let entries = map
            .into_iter()
            .filter(|(name, _)| dir.join("icons").join(format!("{name}.svg")).is_file())
            .map(|(n, t)| (n, t.into_iter().map(|s| s.to_lowercase()).collect()))
            .collect();
        Ok(Self { entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Tìm icon. Mỗi từ (hoặc cụm tiếng Việt) trong `query` phải khớp tên hoặc thẻ; xếp theo độ khớp.
    pub fn search(&self, query: &str, limit: usize) -> Vec<IconHit> {
        let hit = |n: &str| IconHit {
            name: n.to_string(),
            src: format!("builtin:{LUCIDE_DIR}/icons/{n}.svg"),
        };
        let groups = expand(query);
        if groups.is_empty() {
            return DEFAULT
                .iter()
                .filter(|n| self.entries.iter().any(|(e, _)| e == *n))
                .map(|n| hit(n))
                .collect();
        }
        let mut scored: Vec<(i32, &str)> = self
            .entries
            .iter()
            .filter_map(|(name, tags)| {
                let mut total = 0;
                for alts in &groups {
                    let best = alts.iter().map(|t| score(name, tags, t)).max().unwrap_or(0);
                    if best == 0 {
                        return None;
                    }
                    total += best;
                }
                Some((total, name.as_str()))
            })
            .collect();
        // điểm cao trước; cùng điểm → tên ngắn (icon "gốc") trước
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
        scored.into_iter().take(limit).map(|(_, n)| hit(n)).collect()
    }
}

/// Mỗi nhóm = các cách viết thay thế cho một từ của người dùng (từ gốc + bản dịch tiếng Anh).
fn expand(query: &str) -> Vec<Vec<String>> {
    let mut rest = fold(query).split_whitespace().collect::<Vec<_>>().join(" ");
    let mut groups = Vec::new();
    // cụm tiếng Việt dài trước ("mui ten" trước "ten")
    let mut vi: Vec<&(&str, &str)> = VI.iter().collect();
    vi.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    for (k, en) in vi {
        let padded = format!(" {rest} ");
        let needle = format!(" {k} ");
        if padded.contains(&needle) {
            rest = padded.replacen(&needle, " ", 1).trim().to_string();
            let mut alts: Vec<String> = en.split_whitespace().map(str::to_string).collect();
            alts.push(k.to_string());
            groups.push(alts);
        }
    }
    for w in rest.split_whitespace() {
        groups.push(vec![w.to_string()]);
    }
    groups
}

fn score(name: &str, tags: &[String], t: &str) -> i32 {
    let words = || name.split('-');
    if name == t {
        100
    } else if words().next() == Some(t) {
        60
    } else if words().any(|w| w == t) {
        45
    } else if name.contains(t) {
        25
    } else if tags.iter().any(|g| g == t) {
        20
    } else if t.len() >= 3 && tags.iter().any(|g| g.contains(t)) {
        8
    } else {
        0
    }
}

/// Thư mục bộ icon Lucide trong `assets/`.
pub fn lucide_dir(assets: &Path) -> PathBuf {
    assets.join(LUCIDE_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> IconIndex {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(LUCIDE_DIR);
        IconIndex::load(&dir).unwrap()
    }

    #[test]
    fn english_and_vietnamese_queries_find_icons() {
        let idx = index();
        assert!(idx.len() > 1000);
        let names = |q: &str| idx.search(q, 20).into_iter().map(|h| h.name).collect::<Vec<_>>();
        assert_eq!(names("house")[0], "house");
        assert!(
            names("mũi tên phải").iter().any(|n| n == "arrow-right"),
            "{:?}",
            names("mũi tên phải")
        );
        assert!(names("mui ten").iter().all(|n| n.contains("arrow")));
        assert!(names("bóng đèn").contains(&"lightbulb".to_string()));
        assert!(names("nhà").contains(&"house".to_string()));
        assert!(names("không có icon nào như thế này zzz").is_empty());
        // chưa gõ → gợi ý, toàn icon có thật
        let d = idx.search("", 50);
        assert!(d.len() > 20 && d[0].name == "arrow-right");
        assert!(d[0].src.starts_with("builtin:icons/lucide/icons/"));
    }
}
