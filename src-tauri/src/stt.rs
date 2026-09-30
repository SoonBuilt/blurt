//! On-device speech to text: Parakeet TDT 0.6B v3 (NVIDIA, CC-BY-4.0) on ONNX Runtime,
//! on both macOS and Windows. The model comes from the `speech` pack (see models.rs).

use crate::models::{self, Pack};
use parking_lot::Mutex;
use std::path::Path;
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};
use transcribe_rs::onnx::Quantization;

pub struct Transcriber {
    model: Mutex<Option<ParakeetModel>>,
}

impl Transcriber {
    pub fn new() -> Self {
        Self { model: Mutex::new(None) }
    }

    /// Loads the model into memory (a second or two); call off the main thread.
    pub fn load(&self, data_dir: &Path) -> anyhow::Result<()> {
        let mut slot = self.model.lock();
        if slot.is_none() {
            let m = ParakeetModel::load(&models::dir(data_dir, Pack::Speech), &Quantization::Int8)
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
