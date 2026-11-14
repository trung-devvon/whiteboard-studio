//! SPIKE S4 — Viết chữ tiếng Việt bằng "xương glyph + reveal".
//!
//! Câu hỏi: với một font viết tay bình thường (đường bao, không phải font một nét), có lấy được
//! đường tâm để bàn tay đi theo, tách được dấu thanh để viết sau, và reveal phủ kín chữ không?
//! Đạt nếu: (1) phát hiện đủ thành phần dấu, (2) mọi nét dấu đi sau nét chữ trong cùng từ,
//! (3) mặt nạ reveal phủ ≥ 99% điểm ảnh chữ.
//!
//! Chạy: `cargo run -p s4-glyph-skeleton --profile bench-fast [đường_dẫn_font.ttf]`
//! Mặc định dùng Segoe Print của Windows (chỉ để thử nghiệm — KHÔNG đóng gói font này).

mod skeleton;

use anyhow::{Context, Result};
use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use skeleton::{Bitmap, Pt, distance_transform, rdp, trace_strokes, zhang_suen};
use spike_common::{FfmpegWriter, out_dir};
use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};
use unicode_normalization::UnicodeNormalization;

/// Chiều cao em (px) khi rasterize glyph. Cao → xương mượt hơn nhưng chậm hơn.
const EM_PX: f32 = 220.0;
const MARGIN: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Base,
    Diacritic,
}

struct PlannedStroke {
    word: usize,
    kind: Kind,
    pts: Vec<Pt>, // toạ độ canvas, w = bán kính
}

struct PathSink {
    pb: PathBuilder,
    scale: f32,
    ox: f32,
    oy: f32,
}
impl OutlineBuilder for PathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.ox + x * self.scale, self.oy - y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.ox + x * self.scale, self.oy - y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(
            self.ox + x1 * self.scale,
            self.oy - y1 * self.scale,
            self.ox + x * self.scale,
            self.oy - y * self.scale,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.ox + x1 * self.scale,
            self.oy - y1 * self.scale,
            self.ox + x2 * self.scale,
            self.oy - y2 * self.scale,
            self.ox + x * self.scale,
            self.oy - y * self.scale,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

struct LineResult {
    canvas_w: u32,
    canvas_h: u32,
    glyph_fill: Bitmap, // toàn bộ chữ (để đo độ phủ)
    strokes: Vec<PlannedStroke>,
    expected_marks: usize,
    found_marks: usize,
}

/// Số dấu "kỳ vọng" của một chuỗi: dấu tổ hợp sau NFD + chấm của i/j.
fn expected_mark_count(text: &str) -> usize {
    let d: Vec<char> = text.nfd().collect();
    let mut n = 0;
    for (k, c) in d.iter().enumerate() {
        match c {
            // râu của ơ/ư (U+031B) dính liền thân chữ → không tính
            '\u{031B}' => {}
            '\u{0300}'..='\u{036F}' => n += 1,
            // chấm của i/j — trừ khi bị thay bằng dấu huyền/sắc/ngã/hỏi (ì í ĩ ỉ); ị vẫn giữ chấm trên
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

fn process_line(face: &rustybuzz::Face, text: &str) -> Result<LineResult> {
    let text: String = text.nfc().collect();
    // Font thiếu glyph tiếng Việt → báo lỗi rõ ràng thay vì vẽ ô vuông (.notdef)
    let missing: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && face.glyph_index(*c).is_none())
        .collect();
    anyhow::ensure!(
        missing.is_empty(),
        "font THIẾU glyph cho: \"{missing}\" — không dùng được cho tiếng Việt"
    );
    let upem = face.units_per_em() as f32;
    let scale = EM_PX / upem;
    let ascender = face.ascender() as f32 * scale;
    let descender = face.descender() as f32 * scale; // âm
    let x_height = face.x_height().map(|v| v as f32).unwrap_or(upem * 0.5) * scale;

    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(&text);
    let shaped = rustybuzz::shape(face, &[], buf);
    let infos = shaped.glyph_infos();
    let poss = shaped.glyph_positions();

    let total_adv: f32 = poss.iter().map(|p| p.x_advance as f32 * scale).sum();
    let canvas_w = ((total_adv + 2.0 * MARGIN) as u32 + 1) & !1;
    let canvas_h = ((ascender - descender + 2.0 * MARGIN) as u32 + EM_PX as u32 / 3 + 1) & !1;
    let baseline = MARGIN + ascender + EM_PX / 6.0;

    let mut glyph_fill = Bitmap::new(canvas_w as usize, canvas_h as usize);
    let mut base_strokes: Vec<Vec<(usize, Vec<Pt>)>> = Vec::new(); // theo từ
    let mut mark_strokes: Vec<Vec<(usize, Vec<Pt>)>> = Vec::new();
    let mut found_marks = 0;
    let mut pen_x = MARGIN;

    for (info, pos) in infos.iter().zip(poss) {
        let word = text[..info.cluster as usize]
            .chars()
            .filter(|c| c.is_whitespace())
            .count();
        while base_strokes.len() <= word {
            base_strokes.push(Vec::new());
            mark_strokes.push(Vec::new());
        }
        let ox = pen_x + pos.x_offset as f32 * scale;
        let oy = baseline - pos.y_offset as f32 * scale;
        pen_x += pos.x_advance as f32 * scale;

        // 1) Rasterize riêng từng glyph
        let mut sink = PathSink {
            pb: PathBuilder::new(),
            scale,
            ox,
            oy,
        };
        if face.outline_glyph(GlyphId(info.glyph_id as u16), &mut sink).is_none() {
            continue; // dấu cách
        }
        let Some(path) = sink.pb.finish() else { continue };
        let mut pm = Pixmap::new(canvas_w, canvas_h).unwrap();
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.anti_alias = true;
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        let glyph = Bitmap::from_alpha(&pm, 128);
        glyph_fill.or_assign(&glyph);

        // 2) Tách thành phần liên thông → phân loại thân chữ / dấu
        let comps = glyph.components();
        let dist = distance_transform(&glyph);
        // Độ cao (px, so với baseline, hướng lên) của mép dưới / mép trên mỗi thành phần
        let span = |c: &skeleton::Comp| (baseline - c.y_max as f32, baseline - c.y_min as f32);
        // Glyph có "thân" nếu một thành phần vắt qua vùng giữa baseline và x-height
        let has_body = comps.iter().any(|c| {
            let (lo, hi) = span(c);
            lo < 0.5 * x_height && hi > 0.3 * x_height
        });
        for comp in &comps {
            let (lo, hi) = span(comp);
            let above = lo > 0.80 * x_height; // nằm hẳn trên x-height: sắc, huyền, hỏi, ngã, mũ, trăng, chấm i
            let below = hi < 0.05 * x_height; // nằm hẳn dưới baseline: dấu nặng
            let kind = if has_body && (above || below) {
                Kind::Diacritic
            } else {
                Kind::Base
            };
            if kind == Kind::Diacritic {
                found_marks += 1;
            }
            // 3) Rút xương thành phần → nét
            let mask = glyph.only_label(comp);
            let skel = zhang_suen(&mask);
            let mut strokes = trace_strokes(&skel, &dist);
            // Chấm nhỏ (chấm chữ i, dấu nặng): xương chỉ còn 1 điểm → tạo "nét chấm" ngắn với bán kính đủ phủ chấm
            let reach: f32 = strokes.iter().flat_map(|s| s.iter().map(|p| p.w)).fold(0.0, f32::max);
            if strokes.is_empty() || (comp.area as f32) < std::f32::consts::PI * (reach + 2.0).powi(2) {
                let c = glyph.dot_of(comp, &dist);
                let r = ((comp.area as f32) / std::f32::consts::PI).sqrt();
                let c = Pt { w: c.w.max(r), ..c };
                strokes = vec![vec![Pt { x: c.x - 0.5, ..c }, Pt { x: c.x + 0.5, ..c }]];
            }
            let target = if kind == Kind::Base {
                &mut base_strokes[word]
            } else {
                &mut mark_strokes[word]
            };
            for s in strokes {
                let mut s = rdp(&s, 0.8);
                extend_free_ends(&mut s);
                target.push((comp.x_min, s));
            }
        }
    }

    // 4) Thứ tự viết: theo từ — hết thân chữ (trái → phải, láng giềng gần nhất) rồi mới tới dấu
    let mut strokes = Vec::new();
    for word in 0..base_strokes.len() {
        for (list, kind) in [
            (&mut base_strokes[word], Kind::Base),
            (&mut mark_strokes[word], Kind::Diacritic),
        ] {
            for pts in order_strokes(std::mem::take(list)) {
                strokes.push(PlannedStroke { word, kind, pts });
            }
        }
    }
    Ok(LineResult {
        canvas_w,
        canvas_h,
        glyph_fill,
        strokes,
        expected_marks: expected_mark_count(&text),
        found_marks,
    })
}

/// Zhang–Suen luôn "gặm" đầu nét một đoạn ≈ bán kính nét → kéo dài hai đầu theo tiếp tuyến.
/// (Phần lố ra ngoài chữ vô hại vì reveal chỉ hiện điểm ảnh của chữ.)
fn extend_free_ends(s: &mut Vec<Pt>) {
    if s.len() < 2 || s[0].dist(s.last().unwrap()) < 1.0 {
        return; // vòng kín hoặc nét chấm
    }
    let ext = |a: Pt, b: Pt| -> Pt {
        let d = a.dist(&b).max(1e-3);
        let k = 0.9 * a.w / d;
        Pt {
            x: a.x + (a.x - b.x) * k,
            y: a.y + (a.y - b.y) * k,
            w: a.w,
        }
    };
    let head = ext(s[0], s[1]);
    let n = s.len();
    let tail = ext(s[n - 1], s[n - 2]);
    s.insert(0, head);
    s.push(tail);
}

/// Sắp nét: ưu tiên trái → phải theo thành phần, trong đó láng giềng gần nhất, cho phép đảo chiều nét.
fn order_strokes(mut items: Vec<(usize, Vec<Pt>)>) -> Vec<Vec<Pt>> {
    items.retain(|(_, s)| s.len() >= 2);
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Option<Pt> = None;
    while !items.is_empty() {
        let (idx, rev) = match cur {
            None => {
                // nét đầu: thành phần trái nhất, đầu nét trên-trái nhất
                let i = (0..items.len()).min_by_key(|&i| (items[i].0, 0)).unwrap();
                let s = &items[i].1;
                let score = |p: &Pt| p.y * 1.5 + p.x;
                (i, score(s.last().unwrap()) < score(&s[0]))
            }
            Some(c) => {
                let mut best = (0, false, f32::MAX);
                for (i, (cx, s)) in items.iter().enumerate() {
                    // phạt nhẹ việc nhảy sang thành phần ở xa bên phải (giữ trái → phải)
                    let bias = (*cx as f32 - c.x).max(0.0) * 0.3;
                    let d0 = s[0].dist(&c) + bias;
                    let d1 = s.last().unwrap().dist(&c) + bias;
                    if d0 < best.2 {
                        best = (i, false, d0)
                    }
                    if d1 < best.2 {
                        best = (i, true, d1)
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

/// Vẽ đoạn nét [a,b] vào mặt nạ reveal với bán kính theo độ dày nét (+ biên an toàn).
fn stamp(mask: &mut Pixmap, a: &Pt, b: &Pt) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.x, a.y);
    pb.line_to(b.x + 0.01, b.y);
    let Some(path) = pb.finish() else { return };
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.anti_alias = true;
    let width = 2.0 * a.w.max(b.w) + 3.0;
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    mask.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

fn coverage(line: &LineResult) -> f32 {
    let mut mask = Pixmap::new(line.canvas_w, line.canvas_h).unwrap();
    for s in &line.strokes {
        for seg in s.pts.windows(2) {
            stamp(&mut mask, &seg[0], &seg[1]);
        }
    }
    let m = Bitmap::from_alpha(&mask, 128);
    let total = line.glyph_fill.count();
    let covered = line.glyph_fill.and_count(&m);
    covered as f32 / total.max(1) as f32
}

fn save_debug_png(line: &LineResult, path: &std::path::Path) -> Result<()> {
    let mut pm = Pixmap::new(line.canvas_w, line.canvas_h).unwrap();
    pm.fill(Color::WHITE);
    for (i, a) in line.glyph_fill.data.iter().enumerate() {
        if *a {
            let px = &mut pm.pixels_mut()[i];
            *px = tiny_skia::ColorU8::from_rgba(225, 225, 225, 255).premultiply();
        }
    }
    let n = line.strokes.len().max(1) as f32;
    for (i, s) in line.strokes.iter().enumerate() {
        let t = i as f32 / n;
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        match s.kind {
            Kind::Base => paint.set_color_rgba8(
                (40.0 + 40.0 * t) as u8,
                (90.0 + 120.0 * t) as u8,
                (230.0 - 150.0 * t) as u8,
                255,
            ),
            Kind::Diacritic => paint.set_color_rgba8(230, 40, 40, 255),
        }
        let mut pb = PathBuilder::new();
        pb.move_to(s.pts[0].x, s.pts[0].y);
        for p in &s.pts[1..] {
            pb.line_to(p.x, p.y);
        }
        if let Some(path) = pb.finish() {
            pm.stroke_path(
                &path,
                &paint,
                &Stroke {
                    width: 3.0,
                    ..Stroke::default()
                },
                Transform::identity(),
                None,
            );
        }
        if let Some(dot) = PathBuilder::from_circle(s.pts[0].x, s.pts[0].y, 4.5) {
            paint.set_color_rgba8(0, 0, 0, 255);
            pm.fill_path(&dot, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    pm.save_png(path)?;
    Ok(())
}

/// Video demo: bàn tay (chấm tròn) đi theo nét với tốc độ đều, mặt nạ reveal mở dần chữ gốc.
fn save_video(line: &LineResult, path: &std::path::Path) -> Result<()> {
    const FPS: u32 = 30;
    const SPEED: f32 = 900.0; // px/s khi đặt bút
    const LIFT_S: f32 = 0.12; // thời gian nhấc bút giữa 2 nét
    let (w, h) = (line.canvas_w, line.canvas_h);
    // Ảnh nguồn "mực": chữ đen đặc
    let mut ink = Pixmap::new(w, h).unwrap();
    for (i, a) in line.glyph_fill.data.iter().enumerate() {
        if *a {
            ink.pixels_mut()[i] = tiny_skia::ColorU8::from_rgba(20, 20, 30, 255).premultiply();
        }
    }
    // Dòng thời gian: mỗi nét có (bắt đầu, kết thúc)
    let mut t = 0.3f32;
    let mut sched = Vec::new();
    for s in &line.strokes {
        let len: f32 = s.pts.windows(2).map(|p| p[0].dist(&p[1])).sum();
        let dur = (len / SPEED).max(0.05);
        sched.push((t, t + dur, len));
        t += dur + LIFT_S;
    }
    let total = t + 0.8;
    let frames = (total * FPS as f32) as u32;
    let mut ff = FfmpegWriter::spawn(w, h, FPS, &["-c:v", "libopenh264", "-b:v", "6M"], path)?;
    let mut mask = Pixmap::new(w, h).unwrap();
    let mut done_len: Vec<f32> = vec![0.0; line.strokes.len()];
    let mut frame = Pixmap::new(w, h).unwrap();
    for f in 0..frames {
        let now = f as f32 / FPS as f32;
        let mut pen: Option<Pt> = None;
        for (i, s) in line.strokes.iter().enumerate() {
            let (t0, t1, len) = sched[i];
            if now < t0 {
                break;
            }
            // minimum-jerk: vị trí dọc nét theo s(τ)=10τ³−15τ⁴+6τ⁵
            let tau = ((now - t0) / (t1 - t0)).clamp(0.0, 1.0);
            let target = len * (10.0 * tau.powi(3) - 15.0 * tau.powi(4) + 6.0 * tau.powi(5));
            if target > done_len[i] {
                let mut acc = 0.0;
                for seg in s.pts.windows(2) {
                    let l = seg[0].dist(&seg[1]);
                    if acc + l > done_len[i] && acc < target {
                        let a = seg[0].lerp(&seg[1], ((done_len[i] - acc) / l).clamp(0.0, 1.0));
                        let b = seg[0].lerp(&seg[1], ((target - acc) / l).clamp(0.0, 1.0));
                        stamp(&mut mask, &a, &b);
                    }
                    acc += l;
                }
                done_len[i] = target;
            }
            if now <= t1 + LIFT_S {
                let mut acc = 0.0;
                let mut p = *s.pts.last().unwrap();
                for seg in s.pts.windows(2) {
                    let l = seg[0].dist(&seg[1]);
                    if acc + l >= target {
                        p = seg[0].lerp(&seg[1], ((target - acc) / l).clamp(0.0, 1.0));
                        break;
                    }
                    acc += l;
                }
                pen = Some(p);
            }
        }
        frame.fill(Color::WHITE);
        let mut ink_masked = ink.clone();
        let m = tiny_skia::Mask::from_pixmap(mask.as_ref(), tiny_skia::MaskType::Alpha);
        ink_masked.apply_mask(&m);
        frame.draw_pixmap(
            0,
            0,
            ink_masked.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        if let Some(p) = pen
            && let Some(c) = PathBuilder::from_circle(p.x + 14.0, p.y + 14.0, 16.0)
        {
            let mut paint = Paint::default();
            paint.set_color_rgba8(240, 190, 150, 220);
            paint.anti_alias = true;
            frame.fill_path(&c, &paint, FillRule::Winding, Transform::identity(), None);
        }
        ff.write_frame(frame.data())?;
    }
    ff.finish()
}

fn main() -> Result<()> {
    let font_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\Windows\Fonts\segoepr.ttf".into());
    let data = std::fs::read(&font_path).with_context(|| format!("không đọc được font {font_path}"))?;
    let face = rustybuzz::Face::from_slice(&data, 0).context("font không hợp lệ")?;
    let font_name = std::path::Path::new(&font_path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    println!("SPIKE S4 — font: {font_path}");

    let lines = ["Tiếng Việt", "Chào mừng các bạn", "Học hỏi chăm chỉ"];
    let mut report = format!(
        "## S4 — {font_name}\n\n| Dòng | Số nét | Dấu phát hiện / kỳ vọng | Dấu sau thân chữ | Độ phủ reveal | Thời gian |\n|---|---|---|---|---|---|\n"
    );
    let mut all_ok = true;
    for (li, text) in lines.iter().enumerate() {
        let t0 = std::time::Instant::now();
        let line = match process_line(&face, text) {
            Ok(l) => l,
            Err(e) => {
                println!("  • \"{text}\": {e}");
                report += &format!("| {text} | — | — | — | — | {e} |\n");
                all_ok = false;
                continue;
            }
        };
        let ms = t0.elapsed().as_millis();
        // Kiểm tra: trong mỗi từ, không có nét thân chữ nào đi sau một nét dấu
        let order_ok = line
            .strokes
            .windows(2)
            .all(|w| !(w[0].word == w[1].word && w[0].kind == Kind::Diacritic && w[1].kind == Kind::Base));
        let cov = coverage(&line);
        let marks_ok = line.found_marks >= line.expected_marks;
        let ok = order_ok && cov >= 0.99 && marks_ok;
        all_ok &= ok;
        println!(
            "  • \"{text}\": {} nét · dấu {}/{} · thứ tự {} · phủ {:.2}% · {ms}ms → {}",
            line.strokes.len(),
            line.found_marks,
            line.expected_marks,
            if order_ok { "OK" } else { "SAI" },
            cov * 100.0,
            if ok { "ĐẠT" } else { "CHƯA ĐẠT" }
        );
        report += &format!(
            "| {text} | {} | {}/{} | {} | {:.2}% | {ms}ms |\n",
            line.strokes.len(),
            line.found_marks,
            line.expected_marks,
            if order_ok { "✅" } else { "❌" },
            cov * 100.0
        );
        save_debug_png(&line, &out_dir().join(format!("s4_{font_name}_{li}.png")))?;
        if li == 0 {
            match save_video(&line, &out_dir().join(format!("s4_{font_name}_demo.mp4"))) {
                Ok(()) => println!("    video demo: s4_{font_name}_demo.mp4"),
                Err(e) => println!("    (bỏ qua video: {e})"),
            }
        }
    }
    report += &format!(
        "\nKết luận: {}\n",
        if all_ok {
            "ĐẠT"
        } else {
            "CHƯA ĐẠT — xem ảnh debug"
        }
    );
    std::fs::write(out_dir().join(format!("s4-report-{font_name}.md")), report)?;
    println!("Ảnh debug + báo cáo trong {}", out_dir().display());
    Ok(())
}
