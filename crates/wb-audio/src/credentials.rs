//! Key Azure Speech của người dùng: cất trong kho mật khẩu của hệ điều hành (Windows Credential Manager),
//! không ghi vào file dự án hay git. Khi phát triển có thể dùng biến môi trường / file `.env` thay thế.

use crate::AzureConfig;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "whiteboard-studio";
const ACCOUNT: &str = "azure-speech";

/// Key đang dùng lấy từ đâu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Kho mật khẩu hệ điều hành (nhập trong app).
    Keyring,
    /// Biến môi trường / file `.env` (khi phát triển).
    Env,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    key: String,
    region: String,
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
}

/// Key đã lưu trong app trước, rồi tới biến môi trường / `.env`.
pub fn load() -> Option<(AzureConfig, Source)> {
    if let Some(cfg) = entry()
        .ok()
        .and_then(|e| e.get_password().ok())
        .and_then(|s| serde_json::from_str::<Stored>(&s).ok())
        .filter(|s| !s.key.is_empty() && !s.region.is_empty())
    {
        return Some((
            AzureConfig {
                key: cfg.key,
                region: cfg.region,
            },
            Source::Keyring,
        ));
    }
    AzureConfig::from_env().map(|c| (c, Source::Env))
}

pub fn save(cfg: &AzureConfig) -> Result<(), String> {
    let json = serde_json::to_string(&Stored {
        key: cfg.key.trim().to_string(),
        region: cfg.region.trim().to_lowercase(),
    })
    .map_err(|e| e.to_string())?;
    entry()?.set_password(&json).map_err(|e| e.to_string())
}

/// Xoá key đã lưu trong app (không đụng tới `.env`).
pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
