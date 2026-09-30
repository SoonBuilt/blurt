//! Hearing how the user sounds. emotion2vec+ base (Ma et al., ACL 2024; FunASR model
//! licence, which asks for attribution) turns the recording into one of nine emotions.
//! Blurt only acts on clear signals, and never says "you sound angry" unless asked.

use crate::models::{self, Pack};
use ort::session::Session;
use parking_lot::Mutex;
use serde::Serialize;
use std::path::Path;

/// Longest stretch of audio analysed: the end of what the user said carries the tone.
const MAX_SECONDS: usize = 10;
/// Below this confidence we treat the tone as neutral.
const MIN_CONFIDENCE: f32 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    /// Clearly neutral: shown to the user, but doesn't change how the AI writes.
    Calm,
    Frustrated,
    Upbeat,
    Down,
    Anxious,
    Surprised,
}

impl Tone {
    /// How the AI should hear it: plain words it can act on.
    pub fn describe(self) -> &'static str {
        match self {
            Tone::Calm => "calm",
            Tone::Frustrated => "frustrated or annoyed",
            Tone::Upbeat => "upbeat and happy",
            Tone::Down => "down or sad",
            Tone::Anxious => "anxious or stressed",
            Tone::Surprised => "surprised",
        }
    }
}

struct Model {
    session: Session,
    weight: Vec<Vec<f32>>,
    bias: Vec<f32>,
    labels: Vec<String>,
}

pub struct ToneDetector {
    model: Mutex<Option<Model>>,
}

impl ToneDetector {
    pub fn new() -> Self {
        Self { model: Mutex::new(None) }
    }

    pub fn is_ready(data_dir: &Path) -> bool {
        models::is_ready(data_dir, Pack::Tone)
    }

    fn load(&self, data_dir: &Path) -> anyhow::Result<()> {
        let mut slot = self.model.lock();
        if slot.is_some() {
            return Ok(());
        }
        let d = models::dir(data_dir, Pack::Tone);
        let head: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.join("emotion2vec_head.json"))?)?;
        let floats = |v: &serde_json::Value| -> Vec<f32> {
            v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default()
        };
        let weight = head["weight"].as_array().map(|rows| rows.iter().map(floats).collect()).unwrap_or_default();
        let bias = floats(&head["bias"]);
        let labels = head["labels"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let session = super::onnx_session(&d.join("emotion2vec_plus_base.onnx"), 2)?;
        *slot = Some(Model { session, weight, bias, labels });
        Ok(())
    }

    /// Warms the model up in the background.
    pub fn warm(&self, data_dir: &Path) {
        if Self::is_ready(data_dir) {
            if let Err(e) = self.load(data_dir) {
                log::error!("tone model: {e:#}");
            }
        }
    }

    /// Returns a clear, non-neutral tone, or None. `samples` are 16 kHz mono.
    pub fn detect(&self, data_dir: &Path, samples: &[f32]) -> Option<Tone> {
        if !Self::is_ready(data_dir) || samples.len() < 16_000 {
            return None;
        }
        self.load(data_dir).map_err(|e| log::error!("tone model: {e:#}")).ok()?;
        let tail = &samples[samples.len().saturating_sub(MAX_SECONDS * 16_000)..];
        let mut slot = self.model.lock();
        let m = slot.as_mut()?;
        let input = ort::value::Tensor::from_array(([1usize, tail.len()], tail.to_vec())).ok()?;
        let name = m.session.inputs()[0].name().to_string();
        let out = m.session.run(ort::inputs![name => input]).ok()?;
        let (shape, feats) = out[0].try_extract_tensor::<f32>().ok()?;
        let (frames, dim) = (shape[1] as usize, shape[2] as usize);
        if frames == 0 {
            return None;
        }
        let mut pooled = vec![0f32; dim];
        for f in 0..frames {
            for d in 0..dim {
                pooled[d] += feats[f * dim + d];
            }
        }
        pooled.iter_mut().for_each(|v| *v /= frames as f32);
        let logits: Vec<f32> = m
            .weight
            .iter()
            .zip(&m.bias)
            .map(|(row, b)| row.iter().zip(&pooled).map(|(w, x)| w * x).sum::<f32>() + b)
            .collect();
        let max = logits.iter().cloned().fold(f32::MIN, f32::max);
        let exp: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
        let sum: f32 = exp.iter().sum();
        let (best, p) = exp.iter().enumerate().map(|(i, e)| (i, e / sum)).max_by(|a, b| a.1.total_cmp(&b.1))?;
        log::debug!("tone: {} {:.2}", m.labels[best], p);
        if p < MIN_CONFIDENCE {
            return None;
        }
        match m.labels[best].as_str() {
            "angry" | "disgusted" => Some(Tone::Frustrated),
            "happy" => Some(Tone::Upbeat),
            "sad" => Some(Tone::Down),
            "fearful" => Some(Tone::Anxious),
            "surprised" => Some(Tone::Surprised),
            "neutral" => Some(Tone::Calm),
            _ => None,
        }
    }
}
