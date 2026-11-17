//! Azure AI Speech qua WebSocket chính thức `wss://<vùng>.tts.speech.microsoft.com/cognitiveservices/websocket/v1`,
//! xác thực bằng key của chính người dùng (header `Ocp-Apim-Subscription-Key`).
//!
//! Vì sao không dùng REST `/cognitiveservices/v1`: REST chỉ trả âm thanh, không có mốc thời gian từ/bookmark.
//! Giao thức tin nhắn (speech.config → synthesis.context → ssml; nhận audio + audio.metadata) theo bộ
//! Speech SDK JavaScript mã nguồn mở (MIT) của Microsoft. Không dùng token ẩn hay vượt DRM (khác Edge-TTS, ADR-0012).

use crate::{Bookmark, SAMPLE_RATE, Speech, WordMark};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// Biến môi trường (hoặc dòng trong file `.env`) chứa key / vùng — dùng khi phát triển và cho CLI.
pub const ENV_KEY: &str = "WB_AZURE_SPEECH_KEY";
pub const ENV_REGION: &str = "WB_AZURE_SPEECH_REGION";

#[derive(Clone)]
pub struct AzureConfig {
    pub key: String,
    /// Vùng của resource Speech, vd. `japaneast`.
    pub region: String,
}

impl std::fmt::Debug for AzureConfig {
    // không bao giờ in key ra log
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureConfig")
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

impl AzureConfig {
    /// Đọc từ biến môi trường; nạp thêm file `.env` (thư mục hiện tại hoặc thư mục cha) nếu có.
    pub fn from_env() -> Option<Self> {
        let _ = dotenvy::dotenv();
        let key = std::env::var(ENV_KEY).ok()?.trim().to_string();
        let region = std::env::var(ENV_REGION).ok()?.trim().to_lowercase();
        (!key.is_empty() && !region.is_empty()).then_some(Self { key, region })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("chưa có key Azure Speech — nhập trong \"Cài đặt giọng đọc\"")]
    NoKey,
    #[error("Azure từ chối key — kiểm tra lại key và vùng (region)")]
    Unauthorized,
    #[error("Azure báo hết hạn mức (gói miễn phí F0: 0,5 triệu ký tự/tháng) hoặc gọi quá nhanh — thử lại sau")]
    Quota,
    #[error("không kết nối được Azure Speech: {0}")]
    Network(String),
    #[error("Azure báo lỗi: {0}")]
    Service(String),
    #[error("lời thoại trống")]
    Empty,
}

/// Đọc SSML → âm thanh PCM 24 kHz mono + mốc thời gian từ và bookmark.
pub fn synthesize(cfg: &AzureConfig, ssml: &str) -> Result<Speech, TtsError> {
    let url = format!(
        "wss://{}.tts.speech.microsoft.com/cognitiveservices/websocket/v1?X-ConnectionId={}",
        cfg.region,
        new_id()
    );
    let mut req = url
        .into_client_request()
        .map_err(|e| TtsError::Network(e.to_string()))?;
    req.headers_mut().insert(
        "Ocp-Apim-Subscription-Key",
        cfg.key.parse().map_err(|_| TtsError::Unauthorized)?,
    );
    let (mut ws, _) = tungstenite::connect(req).map_err(|e| match e {
        tungstenite::Error::Http(r) => match r.status().as_u16() {
            401 | 403 => TtsError::Unauthorized,
            429 => TtsError::Quota,
            s => TtsError::Service(format!("HTTP {s}")),
        },
        e => TtsError::Network(e.to_string()),
    })?;
    set_timeout(&ws, Duration::from_secs(30));

    let rid = new_id();
    let send = |ws: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>, path: &str, ctype: &str, body: &str| {
        let msg = format!(
            "Path: {path}\r\nX-RequestId: {rid}\r\nX-Timestamp: {}\r\nContent-Type: {ctype}\r\n\r\n{body}",
            timestamp()
        );
        ws.send(Message::text(msg))
            .map_err(|e| TtsError::Network(e.to_string()))
    };
    send(
        &mut ws,
        "speech.config",
        "application/json",
        r#"{"context":{"system":{"name":"SpeechSDK","version":"1.0.0","build":"Rust","lang":"Rust"},"os":{"platform":"Windows","name":"whiteboard-studio","version":"0.1"}}}"#,
    )?;
    send(
        &mut ws,
        "synthesis.context",
        "application/json",
        r#"{"synthesis":{"audio":{"metadataOptions":{"bookmarkEnabled":true,"wordBoundaryEnabled":true,"punctuationBoundaryEnabled":false,"sentenceBoundaryEnabled":false,"sessionEndEnabled":true,"visemeEnabled":false},"outputFormat":"raw-24khz-16bit-mono-pcm"},"language":{"autoDetection":false}}}"#,
    )?;
    send(&mut ws, "ssml", "application/ssml+xml", ssml)?;

    let mut speech = Speech::default();
    let mut bytes = Vec::<u8>::new();
    loop {
        let msg = ws.read().map_err(|e| TtsError::Network(e.to_string()))?;
        match msg {
            Message::Text(t) => {
                let (head, body) = t.split_once("\r\n\r\n").unwrap_or((t.as_str(), ""));
                match header(head, "Path") {
                    Some("turn.end") => break,
                    Some("audio.metadata") => read_metadata(body, &mut speech),
                    _ => {}
                }
            }
            Message::Binary(b) => {
                if b.len() < 2 {
                    continue;
                }
                let n = u16::from_be_bytes([b[0], b[1]]) as usize;
                let head = String::from_utf8_lossy(b.get(2..2 + n).unwrap_or_default());
                if header(&head, "Path") == Some("audio") {
                    bytes.extend_from_slice(b.get(2 + n..).unwrap_or_default());
                }
            }
            Message::Close(frame) => {
                let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                return Err(if reason.to_lowercase().contains("quota") || reason.contains("429") {
                    TtsError::Quota
                } else {
                    TtsError::Service(if reason.is_empty() {
                        "đóng kết nối".into()
                    } else {
                        reason
                    })
                });
            }
            _ => {}
        }
    }
    let _ = ws.close(None);
    speech.pcm = bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    speech.duration_ms = speech.pcm.len() as f64 * 1000.0 / SAMPLE_RATE as f64;
    if speech.pcm.is_empty() {
        return Err(TtsError::Service("không nhận được âm thanh".into()));
    }
    Ok(speech)
}

/// `audio.metadata`: mốc tính bằng "tick" 100 ns từ đầu âm thanh.
fn read_metadata(body: &str, speech: &mut Speech) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return;
    };
    let ms = |x: &serde_json::Value| x.as_f64().unwrap_or(0.0) / 10_000.0;
    for m in v["Metadata"].as_array().into_iter().flatten() {
        let d = &m["Data"];
        match m["Type"].as_str() {
            Some("WordBoundary") => speech.words.push(WordMark {
                text: d["text"]["Text"].as_str().unwrap_or_default().to_string(),
                start_ms: ms(&d["Offset"]),
                duration_ms: ms(&d["Duration"]),
            }),
            Some("Bookmark") => speech.bookmarks.push(Bookmark {
                name: d["Bookmark"].as_str().unwrap_or_default().to_string(),
                at_ms: ms(&d["Offset"]),
            }),
            _ => {}
        }
    }
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

fn set_timeout(ws: &WebSocket<MaybeTlsStream<std::net::TcpStream>>, d: Duration) {
    let tcp = match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::NativeTls(s) => s.get_ref(),
        _ => return,
    };
    let _ = tcp.set_read_timeout(Some(d));
}

/// Mã 32 ký tự hex (không gạch) cho X-ConnectionId / X-RequestId.
fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let a = splitmix(nanos ^ std::process::id() as u64);
    let b = splitmix(a ^ SEQ.fetch_add(1, Ordering::Relaxed));
    format!("{a:016x}{b:016x}")
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Thời điểm hiện tại dạng ISO 8601 UTC, vd. `2026-11-18T04:19:07.123Z`.
fn timestamp() -> String {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // đổi số ngày từ 1970-01-01 sang ngày dương lịch (thuật toán civil_from_days của H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60,
        d.subsec_millis()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_ticks_become_milliseconds() {
        let mut s = Speech::default();
        read_metadata(
            r#"{"Metadata":[{"Type":"WordBoundary","Data":{"Offset":1000000,"Duration":2500000,"text":{"Text":"chào","Length":4,"BoundaryType":"WordBoundary"}}},{"Type":"Bookmark","Data":{"Offset":4000000,"Bookmark":"e2"}}]}"#,
            &mut s,
        );
        assert_eq!(s.words[0].text, "chào");
        assert_eq!(s.words[0].start_ms, 100.0);
        assert_eq!(s.words[0].duration_ms, 250.0);
        assert_eq!(s.anchor_of(2), Some(400.0));
    }

    #[test]
    fn ids_and_timestamps_have_the_expected_shape() {
        let a = new_id();
        assert_eq!(a.len(), 32);
        assert_ne!(a, new_id());
        let t = timestamp();
        assert_eq!(t.len(), 24);
        assert!(t.ends_with('Z') && t.starts_with("20"));
        assert_eq!(header("Path: turn.end\r\nX-RequestId: x", "path"), Some("turn.end"));
    }

    /// Gọi Azure thật — chỉ chạy khi có key: `cargo test -p wb-audio -- --ignored`.
    #[test]
    #[ignore]
    fn azure_live_returns_audio_words_and_bookmarks() {
        let cfg = AzureConfig::from_env().expect("cần WB_AZURE_SPEECH_KEY / WB_AZURE_SPEECH_REGION");
        let ssml = crate::narration::ssml(
            "Xin chào các bạn. [1] Đây là bóng đèn, [2] còn đây là ngôi nhà.",
            wb_voice(),
            1.0,
        );
        let s = synthesize(&cfg, &ssml).unwrap();
        assert!(s.duration_ms > 1500.0, "âm thanh quá ngắn: {}", s.duration_ms);
        assert!(s.words.len() >= 8, "ít mốc từ: {:?}", s.words);
        let (a, b) = (s.anchor_of(1).unwrap(), s.anchor_of(2).unwrap());
        assert!(
            0.0 < a && a < b && b < s.duration_ms,
            "mốc sai: {a} {b} / {}",
            s.duration_ms
        );
        eprintln!(
            "audio {:.0} ms, {} từ, [1]={a:.0} ms, [2]={b:.0} ms; từ đầu: {:?}",
            s.duration_ms,
            s.words.len(),
            s.words
                .iter()
                .take(4)
                .map(|w| (&w.text, w.start_ms as u32))
                .collect::<Vec<_>>()
        );
    }

    fn wb_voice() -> &'static str {
        "vi-VN-HoaiMyNeural"
    }
}
