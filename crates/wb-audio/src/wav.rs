//! WAV PCM 16-bit mono tối giản (đủ cho giọng đọc tự tạo — không phải bộ đọc WAV tổng quát).

use std::io;
use std::path::Path;

/// Mã hoá mẫu 16-bit mono thành file WAV trong bộ nhớ.
pub fn encode(pcm: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // cỡ khối fmt
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes()); // byte/giây
    out.extend_from_slice(&2u16.to_le_bytes()); // byte/mẫu
    out.extend_from_slice(&16u16.to_le_bytes()); // bit/mẫu
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Đọc WAV PCM 16-bit mono → (tần số, mẫu).
pub fn decode(bytes: &[u8]) -> io::Result<(u32, Vec<i16>)> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("không phải file WAV"));
    }
    let mut pos = 12;
    let mut rate = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = bytes.get(pos + 8..pos + 8 + len).ok_or_else(|| bad("WAV bị cắt"))?;
        match id {
            b"fmt " => {
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let bits = u16::from_le_bytes([body[14], body[15]]);
                if channels != 1 || bits != 16 {
                    return Err(bad("chỉ hỗ trợ WAV mono 16-bit"));
                }
                rate = Some(u32::from_le_bytes(body[4..8].try_into().unwrap()));
            }
            b"data" => {
                let pcm = body.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
                return Ok((rate.ok_or_else(|| bad("WAV thiếu khối fmt"))?, pcm));
            }
            _ => {}
        }
        pos += 8 + len + (len & 1);
    }
    Err(bad("WAV không có dữ liệu"))
}

pub fn write(path: &Path, pcm: &[i16], rate: u32) -> io::Result<()> {
    std::fs::write(path, encode(pcm, rate))
}

pub fn read(path: &Path) -> io::Result<(u32, Vec<i16>)> {
    decode(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav_roundtrip() {
        let pcm: Vec<i16> = (0..1000).map(|i| ((i * 37) % 2000 - 1000) as i16).collect();
        let (rate, back) = super::decode(&super::encode(&pcm, 24_000)).unwrap();
        assert_eq!(rate, 24_000);
        assert_eq!(back, pcm);
    }
}
