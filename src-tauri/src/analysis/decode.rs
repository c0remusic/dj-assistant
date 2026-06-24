//! In-process audio decode via Symphonia (pure Rust, no FFmpeg spawn).
//! Decodes at the file's NATIVE sample rate — the analysis threads that rate through
//! so cutoff/frequency mapping is correct (no resample lowpass artifact). Channels are
//! forced to mono (1) or stereo (2): mono→mono, stereo passthrough, >2ch downmixed.
//! FFmpeg stays only for the conversion (encode) path elsewhere.

use std::fs::File;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Container/stream format, read from the header without full decode.
pub struct Format {
    pub sample_rate: u32,
    pub channels: u16,
}

/// Outcome of a streamed decode.
pub struct DecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec_error: Option<String>,
}

/// Opens the file and returns a probed format reader.
fn open_format(path: &str) -> Result<Box<dyn FormatReader>, String> {
    let file = File::open(path).map_err(|e| format!("open failed: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("unsupported/unprobeable format: {e}"))?;
    Ok(probed.format)
}

/// First track with a real codec (skips e.g. cover-art "tracks").
fn default_track(format: &dyn FormatReader) -> Result<&Track, String> {
    format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "no decodable audio track".to_string())
}

/// Reads the header and returns the native sample rate + channel count, without decoding audio.
pub fn probe(path: &str) -> Result<Format, String> {
    let format = open_format(path)?;
    let track = default_track(format.as_ref())?;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or("unknown sample rate")?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(0) as u16;
    Ok(Format { sample_rate, channels })
}

/// Decodes `path` at its native sample rate, forcing `target_channels` (1 or 2). Calls
/// `on_block` with interleaved f32 samples. Returns metadata incl. any terminal codec error.
pub fn decode_pcm<F: FnMut(&[f32])>(
    path: &str,
    target_channels: u16,
    mut on_block: F,
) -> Result<DecodeInfo, String> {
    let target = if target_channels >= 2 { 2u16 } else { 1u16 };
    let mut format = open_format(path)?;
    let (track_id, sample_rate, codec_params) = {
        let track = default_track(format.as_ref())?;
        let sr = track.codec_params.sample_rate.ok_or("unknown sample rate")?;
        (track.id, sr, track.codec_params.clone())
    };
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("no decoder for codec: {e}"))?;

    let mut codec_error: Option<String> = None;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut out: Vec<f32> = Vec::with_capacity(8192);

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // Clean end-of-stream: Symphonia surfaces it as an unexpected-EOF IoError.
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => {
                codec_error = Some(e.to_string());
                break;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let sc = spec.channels.count();
                let frames = decoded.frames();
                if sc == 0 || frames == 0 {
                    continue;
                }
                let need = frames * sc;
                if sample_buf.as_ref().map(|b| b.capacity() < need).unwrap_or(true) {
                    sample_buf = Some(SampleBuffer::<f32>::new(frames as u64, spec));
                }
                let buf = sample_buf.as_mut().unwrap();
                buf.copy_interleaved_ref(decoded);
                let samples = buf.samples();

                out.clear();
                if sc == target as usize {
                    out.extend_from_slice(samples);
                } else if target == 1 {
                    for frame in samples.chunks_exact(sc) {
                        out.push(frame.iter().copied().sum::<f32>() / sc as f32);
                    }
                } else if sc == 1 {
                    // mono source, stereo target → duplicate
                    for &s in samples {
                        out.push(s);
                        out.push(s);
                    }
                } else {
                    // >2ch source, stereo target → take front L/R
                    for frame in samples.chunks_exact(sc) {
                        out.push(frame[0]);
                        out.push(frame[1]);
                    }
                }
                if !out.is_empty() {
                    on_block(&out);
                }
            }
            // A single corrupt packet is skippable; keep decoding the rest.
            Err(SymError::DecodeError(_)) => continue,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                codec_error = Some(e.to_string());
                break;
            }
        }
    }

    Ok(DecodeInfo {
        sample_rate,
        channels: target,
        codec_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const F44: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/real_lossless.flac");
    const F48: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fake_lossless_48k.flac");

    #[test]
    fn probe_reports_native_sample_rate() {
        assert_eq!(probe(F44).unwrap().sample_rate, 44100);
        assert_eq!(probe(F48).unwrap().sample_rate, 48000);
    }

    #[test]
    fn decode_pcm_streams_full_native_stereo() {
        let mut total = 0usize;
        let info = decode_pcm(F44, 2, |b| total += b.len()).unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert!(info.codec_error.is_none(), "unexpected codec error: {:?}", info.codec_error);
        // ~10 s * 44100 * 2ch interleaved, 5% tolerance for decoder edge frames
        let expected = 10 * 44100 * 2;
        let lo = (expected as f64 * 0.95) as usize;
        let hi = (expected as f64 * 1.05) as usize;
        assert!(total >= lo && total <= hi, "got {total} interleaved samples, expected ~{expected}");
    }
}
