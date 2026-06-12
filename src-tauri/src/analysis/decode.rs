//! One FFmpeg decode → PCM s16le streamed in chunks, converted to interleaved f32.
//! Channel count is forced to mono (1) or stereo (2): native mono stays mono (phase N/A),
//! everything ≥2 ch is downmixed to stereo (CDJ target). Terminal ffmpeg errors are surfaced.
//!
//! Uses the crate's event iterator (`child.iter()`), which reads ffmpeg's stdout internally
//! and yields `OutputChunk(Vec<u8>)` for raw PCM plus `Log(Error)` / `Error` for failures.

use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};

/// Outcome of a streamed decode.
pub struct DecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec_error: Option<String>,
}

const SAMPLE_RATE: u32 = 44100;

/// Decodes `path`, forcing `target_channels` (1 or 2) at 44.1 kHz s16le. Calls `on_block`
/// with interleaved f32 samples. Returns decode metadata incl. any terminal codec error.
pub fn decode_pcm<F: FnMut(&[f32])>(
    path: &str,
    target_channels: u16,
    mut on_block: F,
) -> Result<DecodeInfo, String> {
    let ch = if target_channels >= 2 { 2 } else { 1 };
    let mut child = FfmpegCommand::new()
        .input(path)
        .args([
            "-vn",
            "-ac", &ch.to_string(),
            "-ar", &SAMPLE_RATE.to_string(),
            "-f", "s16le",
            "-acodec", "pcm_s16le",
        ])
        .output("-")
        .spawn()
        .map_err(|e| format!("ffmpeg spawn failed: {e}"))?;

    let iter = child.iter().map_err(|e| format!("ffmpeg iter failed: {e}"))?;

    let mut carry: Option<u8> = None; // odd byte spanning two chunks
    let mut codec_error: Option<String> = None;
    let mut f32_block: Vec<f32> = Vec::with_capacity(8192);

    for ev in iter {
        match ev {
            FfmpegEvent::OutputChunk(bytes) => {
                f32_block.clear();
                let n = bytes.len();
                let mut i = 0;
                if let Some(lo) = carry.take() {
                    if n >= 1 {
                        let s = i16::from_le_bytes([lo, bytes[0]]);
                        f32_block.push(s as f32 / 32768.0);
                        i = 1;
                    } else {
                        carry = Some(lo);
                    }
                }
                while i + 1 < n {
                    let s = i16::from_le_bytes([bytes[i], bytes[i + 1]]);
                    f32_block.push(s as f32 / 32768.0);
                    i += 2;
                }
                if i + 1 == n {
                    carry = Some(bytes[i]);
                }
                if !f32_block.is_empty() {
                    on_block(&f32_block);
                }
            }
            FfmpegEvent::Log(LogLevel::Error, msg) => codec_error = Some(msg),
            FfmpegEvent::Error(msg) => codec_error = Some(msg),
            _ => {}
        }
    }

    let _ = child.wait();

    Ok(DecodeInfo { sample_rate: SAMPLE_RATE, channels: ch, codec_error })
}
