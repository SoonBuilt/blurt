//! On-device speech to text.
//!
//! Parakeet TDT 0.6B v3 (NVIDIA, CC-BY-4.0) through ONNX Runtime runs on both
//! macOS and Windows. It's downloaded once from Hugging Face on first run.

use futures_util::StreamExt;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};
use transcribe_rs::onnx::Quantization;

pub const MODEL_ID: &str = "parakeet-tdt-0.6b-v3-int8";
const HF_BASE: &str = "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main";
/// (file name, approximate size in bytes, used only for progress before the server replies)
const FILES: &[(&str, u64)] = &[
    ("vocab.txt", 100_000),
    ("config.json", 1_000),
    ("nemo128.onnx", 140_000),
    ("decoder_joint-model.int8.onnx", 18_000_000),
    ("encoder-model.int8.onnx", 652_000_000),
];

pub fn model_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join(MODEL_ID)
}

pub fn is_downloaded(data_dir: &Path) -> bool {
    let dir = model_dir(data_dir);
    FILES.iter().all(|(f, _)| dir.join(f).is_file())
}

/// Downloads any missing model files. `progress(done_bytes, total_bytes)`.
pub async fn download(data_dir: &Path, progress: impl Fn(u64, u64)) -> anyhow::Result<()> {
    let dir = model_dir(data_dir);
    tokio::fs::create_dir_all(&dir).await?;
    let client = reqwest::Client::builder()
        .user_agent("Blurt (soonbuilt.com)")
        .build()?;
    let total: u64 = FILES.iter().map(|(_, s)| s).sum();
    let mut done_before: u64 = 0;

    for (name, approx) in FILES {
        let dest = dir.join(name);
        if dest.is_file() {
            done_before += approx;
            progress(done_before, total);
            continue;
        }
        let resp = client
            .get(format!("{HF_BASE}/{name}"))
            .send()
            .await?
            .error_for_status()?;
        let part = dir.join(format!("{name}.part"));
        let mut file = tokio::fs::File::create(&part).await?;
        let mut stream = resp.bytes_stream();
        let mut got: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            got += chunk.len() as u64;
            progress(done_before + got.min(*approx), total);
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&part, &dest).await?;
        done_before += approx;
    }
    progress(total, total);
    Ok(())
}

pub struct Transcriber {
    model: Mutex<Option<ParakeetModel>>,
}

impl Transcriber {
    pub fn new() -> Self {
        Self {
            model: Mutex::new(None),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.model.lock().is_some()
    }

    /// Loads the model into memory (a second or two); call off the main thread.
    pub fn load(&self, data_dir: &Path) -> anyhow::Result<()> {
        let mut slot = self.model.lock();
        if slot.is_none() {
            let m = ParakeetModel::load(&model_dir(data_dir), &Quantization::Int8)
                .map_err(|e| anyhow::anyhow!("couldn't load the speech model: {e}"))?;
            *slot = Some(m);
        }
        Ok(())
    }

    /// Transcribes 16 kHz mono samples.
    pub fn transcribe(&self, data_dir: &Path, samples: &[f32]) -> anyhow::Result<String> {
        self.load(data_dir)?;
        let mut slot = self.model.lock();
        let model = slot.as_mut().expect("loaded above");
        // Pad with a little silence: Parakeet can drop a final word that ends abruptly.
        let mut padded = samples.to_vec();
        padded.extend(std::iter::repeat(0.0).take(crate::audio::TARGET_RATE as usize / 4));
        let result = model
            .transcribe_with(&padded, &ParakeetParams::default())
            .map_err(|e| anyhow::anyhow!("transcription failed: {e}"))?;
        Ok(result.text.trim().to_string())
    }
}
