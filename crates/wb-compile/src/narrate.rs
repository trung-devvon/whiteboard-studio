//! Tạo giọng đọc cho các scene còn thiếu (gọi Azure), lưu vào `generated/tts/` của dự án.
//! Dùng chung cho CLI (`wb tts`) và app (nút "Tạo giọng đọc").

use crate::{CompileContext, NarrationState, SceneNarration};
use anyhow::{Context, Result};
use wb_audio::{AzureConfig, narration, store, synthesize};
use wb_model::Project;

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct NarrateSummary {
    /// Scene có lời thoại.
    pub scenes: usize,
    /// Scene vừa tạo giọng mới (số còn lại dùng giọng đã có).
    pub created: usize,
    /// Số ký tự đã gửi Azure (tính vào hạn mức miễn phí 0,5 triệu/tháng).
    pub chars: usize,
}

/// Tạo giọng cho mọi scene ở trạng thái `Missing`. `progress(xong, tổng_cần_tạo)`.
pub fn generate(
    project: &Project,
    ctx: &CompileContext,
    cfg: &AzureConfig,
    mut progress: impl FnMut(usize, usize),
) -> Result<NarrateSummary> {
    let voice = &project.settings.voice;
    let all: Vec<SceneNarration> = project
        .scenes
        .iter()
        .map(|sc| SceneNarration::load(ctx, sc, voice))
        .collect();
    let todo: Vec<usize> = (0..all.len())
        .filter(|&i| all[i].state == NarrationState::Missing)
        .collect();
    let mut sum = NarrateSummary {
        scenes: all.iter().filter(|n| n.state != NarrationState::None).count(),
        ..Default::default()
    };
    progress(0, todo.len());
    for (k, &i) in todo.iter().enumerate() {
        let sc = &project.scenes[i];
        let text = sc.narration.as_deref().unwrap_or_default();
        let ssml = narration::ssml(text, &voice.name, voice.rate);
        let name = sc.name.as_deref().unwrap_or(&sc.id);
        let speech = synthesize(cfg, &ssml).with_context(|| format!("tạo giọng cho \"{name}\""))?;
        store::save(&ctx.base_dir, &all[i].key, &speech).context("không ghi được file giọng vào dự án")?;
        sum.created += 1;
        sum.chars += narration::spoken(text).chars().count();
        progress(k + 1, todo.len());
    }
    Ok(sum)
}
