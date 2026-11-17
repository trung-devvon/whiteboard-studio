//! Đệm giọng đã tạo trong thư mục dự án: `generated/tts/<khoá>.wav` + `<khoá>.json` (mốc thời gian).
//! Cùng lời thoại + giọng + tốc độ → dùng lại, không gọi Azure (tiết kiệm hạn mức miễn phí).

use crate::{SAMPLE_RATE, Speech, wav};
use std::io;
use std::path::{Path, PathBuf};

/// Thư mục đệm, tương đối so với thư mục dự án.
pub const TTS_DIR: &str = "generated/tts";

pub fn dir(project_dir: &Path) -> PathBuf {
    project_dir.join(TTS_DIR)
}

pub fn exists(project_dir: &Path, key: &str) -> bool {
    let d = dir(project_dir);
    d.join(format!("{key}.json")).is_file() && d.join(format!("{key}.wav")).is_file()
}

pub fn save(project_dir: &Path, key: &str, speech: &Speech) -> io::Result<()> {
    let d = dir(project_dir);
    std::fs::create_dir_all(&d)?;
    wav::write(&d.join(format!("{key}.wav")), &speech.pcm, SAMPLE_RATE)?;
    // ghi JSON sau cùng: có JSON nghĩa là WAV đã ghi xong
    let json = serde_json::to_vec_pretty(speech).map_err(io::Error::other)?;
    std::fs::write(d.join(format!("{key}.json")), json)
}

pub fn load(project_dir: &Path, key: &str) -> io::Result<Speech> {
    let d = dir(project_dir);
    let mut speech: Speech = serde_json::from_slice(&std::fs::read(d.join(format!("{key}.json")))?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let (rate, pcm) = wav::read(&d.join(format!("{key}.wav")))?;
    if rate != SAMPLE_RATE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "tần số WAV không khớp"));
    }
    speech.pcm = pcm;
    Ok(speech)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bookmark;

    #[test]
    fn save_then_load() {
        let dir = std::env::temp_dir().join(format!("wb-tts-store-{}", std::process::id()));
        let s = Speech {
            duration_ms: 50.0,
            words: vec![],
            bookmarks: vec![Bookmark {
                name: "e1".into(),
                at_ms: 12.5,
            }],
            pcm: vec![1, -2, 3],
        };
        assert!(!exists(&dir, "k"));
        save(&dir, "k", &s).unwrap();
        assert!(exists(&dir, "k"));
        let back = load(&dir, "k").unwrap();
        assert_eq!(back.pcm, s.pcm);
        assert_eq!(back.anchor_of(1), Some(12.5));
        assert_eq!(back.anchor_of(2), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
