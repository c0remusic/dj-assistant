//! In-process audio decode via Symphonia (pure Rust, no FFmpeg spawn).
//! Decodes at the file's NATIVE sample rate — the analysis threads that rate through
//! so cutoff/frequency mapping is correct (no resample lowpass artifact). Channels are
//! forced to mono (1) or stereo (2): mono→mono, stereo passthrough, >2ch downmixed.
//! FFmpeg stays only for the conversion (encode) path elsewhere.

use std::fs::File;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions, CODEC_ID_NULL_AUDIO};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, Track};
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

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
    symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| format!("unsupported/unprobeable format: {e}"))
}

/// First audio track with a real codec (skips e.g. cover-art "tracks").
fn default_track(format: &dyn FormatReader) -> Result<&Track, String> {
    format
        .tracks()
        .iter()
        .find(|t| matches!(&t.codec_params, Some(CodecParameters::Audio(p)) if p.codec != CODEC_ID_NULL_AUDIO))
        .ok_or_else(|| "no decodable audio track".to_string())
}

/// Extract audio codec parameters from a track (must be an audio track).
fn audio_params(track: &Track) -> Result<&AudioCodecParameters, String> {
    match &track.codec_params {
        Some(CodecParameters::Audio(p)) => Ok(p),
        _ => Err("track has no audio codec parameters".to_string()),
    }
}

/// Reads the header and returns the native sample rate + channel count, without decoding audio.
pub fn probe(path: &str) -> Result<Format, String> {
    let format = open_format(path)?;
    let track = default_track(format.as_ref())?;
    let params = audio_params(track)?;
    let sample_rate = params.sample_rate.ok_or("unknown sample rate")?;
    let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(0) as u16;
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
    let (track_id, sample_rate, audio_codec_params) = {
        let track = default_track(format.as_ref())?;
        let params = audio_params(track)?;
        let sr = params.sample_rate.ok_or("unknown sample rate")?;
        (track.id, sr, params.clone())
    };
    let mut decoder: Box<dyn AudioDecoder> = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_codec_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("no decoder for codec: {e}"))?;

    let mut codec_error: Option<String> = None;
    let mut out: Vec<f32> = Vec::with_capacity(8192);

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            // Clean end-of-stream in symphonia 0.6: next_packet returns Ok(None).
            Ok(None) => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => {
                codec_error = Some(e.to_string());
                break;
            }
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let sc = decoded.spec().channels().count();
                let frames = decoded.frames();
                if sc == 0 || frames == 0 {
                    continue;
                }
                // Copy to interleaved f32, converting from any sample format automatically.
                let mut interleaved: Vec<f32> = Vec::with_capacity(frames * sc);
                decoded.copy_to_vec_interleaved::<f32>(&mut interleaved);
                let samples = &interleaved;

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
