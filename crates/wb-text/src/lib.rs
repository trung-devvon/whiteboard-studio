//! **Chữ viết tay tiếng Việt**: font thường (đường bao) → đường tâm nét cho bàn tay → reveal.
//!
//! Quy trình (đã kiểm chứng ở spike S4, nay dùng `harfrust` + `skrifa` thay rustybuzz/ttf-parser):
//! 1. Chuẩn hoá NFC, kiểm tra font có đủ glyph (font thiếu dấu tiếng Việt → báo lỗi, không vẽ ô vuông).
//! 2. Shaping từng từ bằng `harfrust` (dấu thanh được đặt đúng vị trí), xuống dòng theo bề rộng khung.
//! 3. Mỗi glyph: lấy đường bao bằng `skrifa` → rasterize → tách **thành phần liên thông**.
//!    Thành phần nằm hẳn trên x-height hoặc dưới baseline của một glyph có thân → **dấu**.
//! 4. Rút xương Guo–Hall từng thành phần → nét; chấm nhỏ (chấm i, dấu nặng) → "nét chấm".
//! 5. Thứ tự viết: theo từ; trong từ viết **hết thân chữ trước, dấu sau** (như người viết thật).
//!
//! Không I/O — font truyền vào dạng bytes → biên dịch được sang wasm32.

use harfrust::{Buffer, ShapeOptions, ShaperFont};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};
use unicode_normalization::UnicodeNormalization;
use wb_model::{PlannedStroke, PointW, StrokeKind};
use wb_planner::geometry::{extend_ends, rdp, smooth};
use wb_vision::binary::Binary;
use wb_vision::skeleton::{distance_transform, guo_hall, trace_strokes};

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("file font không hợp lệ")]
    InvalidFont,
    #[error("font thiếu glyph cho \"{0}\" — không dùng được cho đoạn chữ này (thường là thiếu dấu tiếng Việt)")]
    MissingGlyphs(String),
    #[error("lỗi shaping: {0}")]
    Shape(String),
}

/// Font đã nạp (giữ bytes; `skrifa` đọc trực tiếp, `harfrust` giữ bản riêng).
pub struct Font {
    data: Vec<u8>,
    shaping: harfrust::Font,
}

impl Font {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, TextError> {
        FontRef::new(&data).map_err(|_| TextError::InvalidFont)?;
        let shaping = harfrust::Font::new(data.clone(), 0).ok_or(TextError::InvalidFont)?;
        Ok(Self { data, shaping })
    }

    fn skrifa(&self) -> FontRef<'_> {
        FontRef::new(&self.data).expect("đã kiểm tra khi nạp")
    }

    /// Các ký tự (không tính khoảng trắng) font không có glyph.
    pub fn missing_chars(&self, text: &str) -> String {
        let cmap = self.skrifa().charmap();
        let mut out = String::new();
        for c in text.nfc() {
            if !c.is_whitespace() && cmap.map(c).is_none() && !out.contains(c) {
                out.push(c);
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Bố cục chữ (đơn vị: px khung tham chiếu).
#[derive(Debug, Clone, Copy)]
pub struct TextLayout {
    /// Cỡ chữ (chiều cao em).
    pub size: f32,
    /// Khoảng cách dòng / cỡ chữ.
    pub line_height: f32,
    /// Bề rộng tối đa một dòng (xuống dòng khi vượt). `None` = không xuống dòng tự động.
    pub max_width: Option<f32>,
    pub align: Align,
}

impl Default for TextLayout {
    fn default() -> Self {
        Self {
            size: 72.0,
            line_height: 1.35,
            max_width: None,
            align: Align::Left,
        }
    }
}

/// Kết quả: lớp phủ chữ (alpha) + nét viết, cùng hệ toạ độ raster.
pub struct TextResult {
    pub width: u32,
    pub height: u32,
    /// Độ phủ chữ từng điểm (0..255) — tô màu chữ theo alpha này để ra lớp mực.
    pub alpha: Vec<u8>,
    /// Nét viết theo thứ tự (loại `Text` / `Diacritic`), px raster.
    pub strokes: Vec<PlannedStroke>,
    /// px raster / px khung tham chiếu.
    pub raster_scale: f32,
    /// Lề quanh chữ (px raster): góc trên-trái của dòng đầu nằm ở (margin, margin).
    pub margin: f32,
    /// Số thành phần được nhận là dấu.
    pub marks: usize,
    /// Độ phủ của nét trên chữ (0..1).
    pub coverage: f32,
}

/// Cỡ em tối thiểu khi rasterize để xương glyph mượt (px).
const MIN_EM_PX: f32 = 160.0;

struct Glyph {
    id: u32,
    x: f32,
    y_off: f32,
}

struct Word {
    glyphs: Vec<Glyph>,
    width: f32,
}

/// Bố cục + tách nét cho một đoạn chữ.
pub fn layout_text(font: &Font, text: &str, opt: &TextLayout) -> Result<TextResult, TextError> {
    let text: String = text.nfc().collect();
    let missing = font.missing_chars(&text);
    if !missing.is_empty() {
        return Err(TextError::MissingGlyphs(missing));
    }
    let r = (MIN_EM_PX / opt.size.max(1.0)).max(1.0);
    let em = opt.size * r;
    let sk = font.skrifa();
    let m = sk.metrics(Size::unscaled(), LocationRef::default());
    let upem = m.units_per_em as f32;
    let k = em / upem;
    let ascent = m.ascent * k;
    let descent = m.descent * k; // âm
    let x_height = m.x_height.unwrap_or(upem * 0.5) * k;
    let shaper = ShaperFont::new(&font.shaping);

    let shape = |s: &str| -> Result<Word, TextError> {
        let mut buf = Buffer::new();
        buf.push_str(s);
        buf.guess_segment_properties();
        harfrust::shape(&shaper, &mut buf, ShapeOptions::new()).map_err(|e| TextError::Shape(format!("{e:?}")))?;
        let mut x = 0.0;
        let mut glyphs = Vec::new();
        for (info, pos) in buf.glyph_infos().iter().zip(buf.glyph_positions()) {
            glyphs.push(Glyph {
                id: info.glyph_id,
                x: x + pos.x_offset as f32 * k,
                y_off: pos.y_offset as f32 * k,
            });
            x += pos.x_advance as f32 * k;
        }
        Ok(Word { glyphs, width: x })
    };
    let space = shape(" ")?.width;

    // xuống dòng (tham lam theo từ)
    let max_w = opt.max_width.map(|w| w * r);
    let mut lines: Vec<Vec<Word>> = Vec::new();
    for para in text.split('\n') {
        let mut line: Vec<Word> = Vec::new();
        let mut line_w = 0.0;
        for w in para.split_whitespace() {
            let word = shape(w)?;
            let add = if line.is_empty() {
                word.width
            } else {
                space + word.width
            };
            if let Some(mw) = max_w
                && !line.is_empty()
                && line_w + add > mw
            {
                lines.push(std::mem::take(&mut line));
                line_w = 0.0;
            }
            line_w += if line.is_empty() {
                word.width
            } else {
                space + word.width
            };
            line.push(word);
        }
        lines.push(line);
    }
    let line_width =
        |l: &[Word]| -> f32 { l.iter().map(|w| w.width).sum::<f32>() + space * l.len().saturating_sub(1) as f32 };
    let content_w = lines.iter().map(|l| line_width(l)).fold(0.0, f32::max);
    let box_w = max_w.unwrap_or(content_w).max(content_w);
    let margin = (em * 0.2).ceil();
    let line_h = em * opt.line_height;
    let width = (box_w + 2.0 * margin).ceil() as u32;
    let height = ((lines.len().max(1) - 1) as f32 * line_h + ascent - descent + 2.0 * margin).ceil() as u32;
    let (wu, hu) = (width as usize, height as usize);

    let mut alpha = vec![0u8; wu * hu];
    let outlines = sk.outline_glyphs();
    let mut word_strokes: Vec<(Vec<Item>, Vec<Item>)> = Vec::new();
    let mut marks = 0;

    for (li, line) in lines.iter().enumerate() {
        let baseline = margin + ascent + li as f32 * line_h;
        let lw = line_width(line);
        let mut pen_x = margin
            + match opt.align {
                Align::Left => 0.0,
                Align::Center => (box_w - lw) / 2.0,
                Align::Right => box_w - lw,
            };
        for word in line {
            let mut base = Vec::new();
            let mut mk = Vec::new();
            for g in &word.glyphs {
                let Some(outline) = outlines.get(GlyphId::new(g.id)) else {
                    continue;
                };
                let (ox, oy) = (pen_x + g.x, baseline - g.y_off);
                let mut pen = Pen {
                    pb: PathBuilder::new(),
                    k,
                    ox,
                    oy,
                };
                if outline
                    .draw(
                        DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                        &mut pen,
                    )
                    .is_err()
                {
                    continue;
                }
                let Some(path) = pen.pb.finish() else { continue };
                let Some(raster) = rasterize(&path) else { continue };
                // ghép vào lớp phủ chung
                for y in 0..raster.h {
                    for x in 0..raster.w {
                        let (cx, cy) = (raster.x0 + x as i32, raster.y0 + y as i32);
                        if cx >= 0 && cy >= 0 && (cx as usize) < wu && (cy as usize) < hu {
                            let d = &mut alpha[cy as usize * wu + cx as usize];
                            *d = (*d).max(raster.alpha[y * raster.w + x]);
                        }
                    }
                }
                marks += split_glyph(&raster, baseline, x_height, &mut base, &mut mk);
            }
            word_strokes.push((base, mk));
            pen_x += word.width + space;
        }
    }

    // thứ tự viết: theo từ — thân chữ trước, dấu sau
    let mut strokes = Vec::new();
    for (base, mk) in word_strokes {
        for (items, kind) in [(base, StrokeKind::Text), (mk, StrokeKind::Diacritic)] {
            for s in order_strokes(items) {
                strokes.push(PlannedStroke::new(kind, finish_stroke(s)));
            }
        }
    }

    let ink = Binary::from_fn(wu, hu, |x, y| alpha[y * wu + x] >= 128);
    let all: Vec<Vec<PointW>> = strokes.iter().map(|s| s.points.clone()).collect();
    let coverage = wb_vision::coverage::coverage(&ink, &all);
    Ok(TextResult {
        width,
        height,
        alpha,
        strokes,
        raster_scale: r,
        margin,
        marks,
        coverage,
    })
}

/// Bút chuyển đường bao font (trục y hướng lên) → PathBuilder (trục y hướng xuống).
struct Pen {
    pb: PathBuilder,
    k: f32,
    ox: f32,
    oy: f32,
}

impl Pen {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.k, self.oy - y * self.k)
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (a, b) = self.p(cx0, cy0);
        let (x, y) = self.p(x, y);
        self.pb.quad_to(a, b, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (a, b) = self.p(cx0, cy0);
        let (c, d) = self.p(cx1, cy1);
        let (x, y) = self.p(x, y);
        self.pb.cubic_to(a, b, c, d, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Glyph đã rasterize trong hộp bao riêng (toạ độ góc trên-trái trên canvas).
struct GlyphRaster {
    x0: i32,
    y0: i32,
    w: usize,
    h: usize,
    alpha: Vec<u8>,
}

fn rasterize(path: &tiny_skia::Path) -> Option<GlyphRaster> {
    let b = path.bounds();
    let x0 = b.left().floor() as i32 - 2;
    let y0 = b.top().floor() as i32 - 2;
    let w = (b.right().ceil() as i32 + 2 - x0).max(1) as u32;
    let h = (b.bottom().ceil() as i32 + 2 - y0).max(1) as u32;
    let mut pm = Pixmap::new(w, h)?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    pm.fill_path(
        path,
        &paint,
        FillRule::Winding,
        Transform::from_translate(-x0 as f32, -y0 as f32),
        None,
    );
    Some(GlyphRaster {
        x0,
        y0,
        w: w as usize,
        h: h as usize,
        alpha: pm.pixels().iter().map(|p| p.alpha()).collect(),
    })
}

/// Một nét chờ sắp thứ tự: (x trái nhất của thành phần, các điểm trên canvas).
type Item = (f32, Vec<PointW>);

/// Thành phần liên thông (8 hướng) của mặt nạ.
fn components(mask: &Binary) -> Vec<Vec<(usize, usize)>> {
    let (w, h) = (mask.w, mask.h);
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for sy in 0..h {
        for sx in 0..w {
            if !mask.get(sx, sy) || seen[sy * w + sx] {
                continue;
            }
            let mut comp = Vec::new();
            let mut stack = vec![(sx, sy)];
            seen[sy * w + sx] = true;
            while let Some((x, y)) = stack.pop() {
                comp.push((x, y));
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            let (nx, ny) = (nx as usize, ny as usize);
                            if mask.get(nx, ny) && !seen[ny * w + nx] {
                                seen[ny * w + nx] = true;
                                stack.push((nx, ny));
                            }
                        }
                    }
                }
            }
            out.push(comp);
        }
    }
    out
}

/// Tách glyph thành thân chữ / dấu, rút xương từng phần. Trả số dấu tìm thấy.
fn split_glyph(g: &GlyphRaster, baseline: f32, x_height: f32, base: &mut Vec<Item>, marks: &mut Vec<Item>) -> usize {
    let mask = Binary::from_fn(g.w, g.h, |x, y| g.alpha[y * g.w + x] >= 128);
    let comps = components(&mask);
    // khoảng (thấp, cao) so với baseline, hướng lên, của mỗi thành phần
    let span = |c: &[(usize, usize)]| {
        let y_min = c.iter().map(|p| p.1).min().unwrap_or(0) as f32 + g.y0 as f32;
        let y_max = c.iter().map(|p| p.1).max().unwrap_or(0) as f32 + g.y0 as f32;
        (baseline - y_max, baseline - y_min)
    };
    let has_body = comps.iter().any(|c| {
        let (lo, hi) = span(c);
        lo < 0.5 * x_height && hi > 0.3 * x_height
    });
    let mut found = 0;
    for comp in &comps {
        let (lo, hi) = span(comp);
        let above = lo > 0.80 * x_height; // sắc, huyền, hỏi, ngã, mũ, trăng, chấm i
        let below = hi < 0.05 * x_height; // dấu nặng
        let is_mark = has_body && (above || below);
        found += is_mark as usize;

        let mut only = Binary::new(g.w, g.h);
        for &(x, y) in comp {
            let i = only.idx(x, y);
            only.data[i] = 1;
        }
        let dist = distance_transform(&only);
        let skel = guo_hall(&only);
        let raw = trace_strokes(&skel, &dist, 1.5, 60.0);
        let to_canvas = |p: &[f32; 3]| PointW::new(p[0] + g.x0 as f32, p[1] + g.y0 as f32, p[2]);
        let mut strokes: Vec<Vec<PointW>> = raw.iter().map(|s| s.iter().map(to_canvas).collect()).collect();
        // chấm nhỏ: xương gần như không có → một "nét chấm" bán kính đủ phủ cả chấm
        let reach = strokes.iter().flat_map(|s| s.iter().map(|p| p.w)).fold(0.0, f32::max);
        let area = comp.len() as f32;
        if strokes.is_empty() || area < std::f32::consts::PI * (reach + 2.0).powi(2) {
            let n = comp.len() as f32;
            let cx = comp.iter().map(|p| p.0 as f32).sum::<f32>() / n + 0.5 + g.x0 as f32;
            let cy = comp.iter().map(|p| p.1 as f32).sum::<f32>() / n + 0.5 + g.y0 as f32;
            let rad = (area / std::f32::consts::PI).sqrt() + 0.5;
            strokes = vec![vec![PointW::new(cx - 0.5, cy, rad), PointW::new(cx + 0.5, cy, rad)]];
        }
        let x_min = comp.iter().map(|p| p.0).min().unwrap_or(0) as f32 + g.x0 as f32;
        let target = if is_mark { &mut *marks } else { &mut *base };
        for s in strokes {
            target.push((x_min, s));
        }
    }
    found
}

/// Kéo dài đầu nét, giản lược, làm mượt — giống planner cho ảnh.
fn finish_stroke(s: Vec<PointW>) -> Vec<PointW> {
    if s.len() <= 2 && s.first().zip(s.last()).is_some_and(|(a, b)| a.dist(b) < 1.5) {
        return s; // nét chấm
    }
    let s = extend_ends(&s, 0.9);
    let s = rdp(&s, 0.6);
    smooth(&s, 2.0, 55.0)
}

/// Sắp nét trong một nhóm: bắt đầu từ thành phần trái nhất (đầu nét trên-trái),
/// sau đó láng giềng gần nhất, cho đảo chiều nét, phạt nhẹ việc nhảy xa sang phải.
fn order_strokes(mut items: Vec<Item>) -> Vec<Vec<PointW>> {
    items.retain(|(_, s)| !s.is_empty());
    let mut out = Vec::new();
    let mut cur: Option<PointW> = None;
    while !items.is_empty() {
        let (idx, rev) = match cur {
            None => {
                let i = (0..items.len())
                    .min_by(|&a, &b| items[a].0.total_cmp(&items[b].0))
                    .unwrap();
                let s = &items[i].1;
                let score = |p: &PointW| p.y * 1.5 + p.x;
                (i, score(s.last().unwrap()) < score(&s[0]))
            }
            Some(c) => {
                let mut best = (0, false, f32::MAX);
                for (i, (cx, s)) in items.iter().enumerate() {
                    let bias = (cx - c.x).max(0.0) * 0.3;
                    let d0 = s[0].dist(&c) + bias;
                    let d1 = s.last().unwrap().dist(&c) + bias;
                    if d0 < best.2 {
                        best = (i, false, d0);
                    }
                    if d1 < best.2 {
                        best = (i, true, d1);
                    }
                }
                (best.0, best.1)
            }
        };
        let (_, mut s) = items.swap_remove(idx);
        if rev {
            s.reverse();
        }
        cur = s.last().copied();
        out.push(s);
    }
    out
}

/// Số dấu "kỳ vọng" của một chuỗi: dấu tổ hợp sau NFD + chấm của i/j (để kiểm thử).
pub fn expected_mark_count(text: &str) -> usize {
    let d: Vec<char> = text.nfd().collect();
    let mut n = 0;
    for (k, c) in d.iter().enumerate() {
        match c {
            '\u{031B}' => {} // râu ơ/ư dính liền thân chữ
            '\u{0300}'..='\u{036F}' => n += 1,
            'i' | 'j' => {
                let replaced = d
                    .get(k + 1)
                    .is_some_and(|m| matches!(m, '\u{0300}' | '\u{0301}' | '\u{0303}' | '\u{0309}'));
                if !replaced {
                    n += 1;
                }
            }
            _ => {}
        }
    }
    n
}

#[cfg(test)]
mod tests;
