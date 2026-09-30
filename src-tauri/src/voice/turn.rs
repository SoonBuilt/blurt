//! Knowing when the user has finished talking, for hands-free mode.
//!
//! A cheap loudness check notices a pause; then Smart Turn v3.2 (Pipecat, BSD-2) listens
//! to the last 8 seconds and decides whether that pause was the end of a thought or just
//! a breath ("so I think we should… um…").

use super::mel;
use crate::models::{self, Pack};
use ort::session::Session;
use parking_lot::Mutex;
use std::path::Path;

const RATE: usize = 16_000;
const WINDOW: usize = 8 * RATE;

pub struct TurnDetector {
    session: Mutex<Option<Session>>,
}

impl TurnDetector {
    pub fn new() -> Self {
        Self { session: Mutex::new(None) }
    }

    fn path(data_dir: &Path) -> std::path::PathBuf {
        models::dir(data_dir, Pack::Speech).join("smart-turn-v3.2-cpu.onnx")
    }

    pub fn is_ready(data_dir: &Path) -> bool {
        Self::path(data_dir).is_file()
    }

    /// Probability (0–1) that the user has finished their turn. `samples` are 16 kHz mono.
    pub fn done_probability(&self, data_dir: &Path, samples: &[f32]) -> anyhow::Result<f32> {
        let mut slot = self.session.lock();
        if slot.is_none() {
            *slot = Some(super::onnx_session(&Self::path(data_dir), 1)?);
        }
        // Last 8 seconds, zero-padded at the front, exactly like the reference implementation.
        let window: Vec<f32> = if samples.len() >= WINDOW {
            samples[samples.len() - WINDOW..].to_vec()
        } else {
            let mut v = vec![0f32; WINDOW - samples.len()];
            v.extend_from_slice(samples);
            v
        };
        let feats = mel::whisper_features(&window);
        let frames = feats.len() / 80;
        let input = ort::value::Tensor::from_array(([1usize, 80, frames], feats))?;
        let session = slot.as_mut().unwrap();
        let out = session.run(ort::inputs!["input_features" => input])?;
        let (_, p) = out[0].try_extract_tensor::<f32>()?;
        Ok(p[0])
    }
}

/// Loudness-based speech tracking over 16 kHz audio, in 30 ms frames.
pub struct Endpoint {
    /// Seconds of trailing quiet.
    pub silence: f32,
    /// Whether any speech has been heard yet.
    pub heard_speech: bool,
}

pub fn endpoint(samples: &[f32]) -> Endpoint {
    const FRAME: usize = RATE * 30 / 1000;
    if samples.len() < FRAME * 4 {
        return Endpoint { silence: 0.0, heard_speech: false };
    }
    let rms: Vec<f32> = samples.chunks(FRAME).map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt()).collect();
    // Adaptive threshold: a little above the quietest tenth of the recording (room noise).
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[sorted.len() / 10];
    let threshold = (floor * 3.0).max(0.008);
    let heard_speech = rms.iter().filter(|&&r| r > threshold * 1.5).count() >= 5;
    let quiet = rms.iter().rev().take_while(|&&r| r <= threshold).count();
    Endpoint { silence: quiet as f32 * 0.03, heard_speech }
}
