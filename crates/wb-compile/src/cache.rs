//! Bộ nhớ đệm kết quả phân tích element (trong RAM, dùng trong app).
//!
//! Khoá = nội dung quyết định kết quả phân tích: toàn bộ `kind` (ảnh + tuỳ chọn, hoặc chữ + font…),
//! bề rộng khung với chữ (ảnh hưởng xuống dòng), và cỡ + thời điểm sửa của file nguồn. Kéo/đổi cỡ một
//! ảnh, đổi thời gian… **không** làm phân tích lại.

use crate::{AnalyzedImage, CompileContext, analyze_element};
use anyhow::Result;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use wb_model::{Element, ElementKind};

/// Giữ tối đa bấy nhiêu kết quả; vượt quá thì xoá kết quả lâu không dùng nhất.
const CAPACITY: usize = 64;

#[derive(Default)]
pub struct AnalysisCache {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    map: HashMap<u64, (Arc<AnalyzedImage>, u64)>,
    clock: u64,
}

impl AnalysisCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Kết quả phân tích của `el`, lấy từ đệm nếu đầu vào không đổi.
    pub fn get_or_analyze(&self, ctx: &CompileContext, el: &Element) -> Result<Arc<AnalyzedImage>> {
        let key = key_of(ctx, el)?;
        {
            let mut g = self.inner.lock().unwrap();
            g.clock += 1;
            let now = g.clock;
            if let Some((a, used)) = g.map.get_mut(&key) {
                *used = now;
                return Ok(a.clone());
            }
        }
        // phân tích ngoài khoá → nhiều element phân tích song song được
        let a = Arc::new(analyze_element(ctx, el)?);
        let mut g = self.inner.lock().unwrap();
        g.clock += 1;
        let now = g.clock;
        if g.map.len() >= CAPACITY
            && let Some(&old) = g.map.iter().min_by_key(|(_, (_, used))| *used).map(|(k, _)| k)
        {
            g.map.remove(&old);
        }
        g.map.insert(key, (a.clone(), now));
        Ok(a)
    }
}

fn key_of(ctx: &CompileContext, el: &Element) -> Result<u64> {
    let mut h = DefaultHasher::new();
    // màu đổi (`tint`) áp lúc lắp gói → không tính vào khoá, đổi màu không phân tích lại
    let mut kind = el.kind.clone();
    if let ElementKind::Image { tint, .. } = &mut kind {
        *tint = None;
    }
    serde_json::to_string(&kind)?.hash(&mut h);
    let file = match &el.kind {
        ElementKind::Image { src, .. } => Some(ctx.resolve(src)?),
        ElementKind::Text { font, .. } => {
            // chữ xuống dòng theo bề rộng khung
            el.transform.w.to_bits().hash(&mut h);
            font.as_deref().map(|f| ctx.resolve(f)).transpose()?
        }
    };
    if let Some(f) = file {
        f.hash(&mut h);
        if let Ok(m) = std::fs::metadata(&f) {
            m.len().hash(&mut h);
            m.modified().ok().hash(&mut h);
        }
    }
    Ok(h.finish())
}
