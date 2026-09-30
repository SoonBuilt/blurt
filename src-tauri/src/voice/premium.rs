//! Premium voice: Chatterbox Turbo (Resemble AI, MIT), 4-bit ONNX, run on the shared ONNX
//! Runtime. More expressive than Pocket TTS and it understands paralinguistic tags such as
//! [laugh], [chuckle] and [sigh]. Tally's voice conditioning is computed once ahead of time
//! (`resources/voices/tally.cbt`), so users don't need the 231 MB speech encoder.
//! The generation loop follows Resemble's reference ONNX example.

use crate::models::{self, Pack};
use ort::memory::Allocator;
use ort::session::{Session, SessionInputValue};
use ort::value::{DynValue, Tensor};
use std::borrow::Cow;
use std::path::Path;

const START: i64 = 6561;
const STOP: i64 = 6562;
const SILENCE: i64 = 4299;
const LAYERS: usize = 24;
const HIDDEN: usize = 1024;
const REPETITION_PENALTY: f32 = 1.2;
pub const SAMPLE_RATE: u32 = 24_000;

/// Tally's voice for Chatterbox: see `dump_conditioning` in the dev tools.
const TALLY_CONDITIONING: &[u8] = include_bytes!("../../resources/voices/tally.cbt");

struct Conditioning {
    features: (Vec<usize>, Vec<f32>),
    prompt_tokens: Vec<i64>,
    speaker_embeddings: (Vec<usize>, Vec<f32>),
    speaker_features: (Vec<usize>, Vec<f32>),
}

pub struct Premium {
    embed: Session,
    lm: Session,
    decoder: Session,
    tokenizer: tokenizers::Tokenizer,
    voice: Conditioning,
}

impl Premium {
    pub fn is_ready(data_dir: &Path) -> bool {
        models::is_ready(data_dir, Pack::Premium)
    }

    pub fn load(data_dir: &Path) -> anyhow::Result<Self> {
        let d = models::dir(data_dir, Pack::Premium);
        let threads = std::thread::available_parallelism().map(|n| n.get().clamp(2, 6)).unwrap_or(4);
        Ok(Self {
            embed: super::onnx_session(&d.join("embed_tokens_q4.onnx"), 2)?,
            lm: super::onnx_session(&d.join("language_model_q4.onnx"), threads)?,
            decoder: super::onnx_session(&d.join("conditional_decoder_q4.onnx"), threads)?,
            tokenizer: tokenizers::Tokenizer::from_file(d.join("tokenizer.json")).map_err(|e| anyhow::anyhow!("{e}"))?,
            voice: parse_conditioning(TALLY_CONDITIONING)?,
        })
    }

    fn embed(&mut self, ids: Vec<i64>) -> anyhow::Result<Vec<f32>> {
        let n = ids.len();
        let x = Tensor::from_array(([1usize, n], ids))?;
        let o = self.embed.run(ort::inputs!["input_ids" => x])?;
        let (_, data) = o[0].try_extract_tensor::<f32>()?;
        Ok(data.to_vec())
    }

    /// Generates speech for one sentence at 24 kHz. `keep_going` is polled between
    /// tokens so the user can interrupt.
    pub fn generate(&mut self, text: &str, keep_going: impl Fn() -> bool) -> anyhow::Result<Option<Vec<f32>>> {
        let ids: Vec<i64> = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .get_ids()
            .iter()
            .map(|&i| i as i64)
            .collect();
        // Roughly 25 speech tokens per second; cap generous for the sentence length.
        let max_tokens = (ids.len() * 20 + 100).min(1000);

        let mut first = self.voice.features.1.clone();
        first.extend(self.embed(ids)?);
        let seq = first.len() / HIDDEN;
        let mut inputs_embeds = Tensor::from_array(([1usize, seq, HIDDEN], first))?.into_dyn();
        let mut mask_len = seq;
        let mut positions: Vec<i64> = (0..seq as i64).collect();
        let empty = || -> anyhow::Result<DynValue> { Ok(Tensor::<f32>::new(&Allocator::default(), [1usize, 16, 0, 64])?.into_dyn()) };
        let mut past: Vec<(String, DynValue)> = Vec::with_capacity(LAYERS * 2);
        for l in 0..LAYERS {
            for kv in ["key", "value"] {
                past.push((format!("past_key_values.{l}.{kv}"), empty()?));
            }
        }
        let mut generated: Vec<i64> = vec![START];

        for _ in 0..max_tokens {
            if !keep_going() {
                return Ok(None);
            }
            let mut feed: Vec<(Cow<str>, SessionInputValue)> = Vec::with_capacity(LAYERS * 2 + 3);
            feed.push(("inputs_embeds".into(), inputs_embeds.into()));
            feed.push(("attention_mask".into(), Tensor::from_array(([1usize, mask_len], vec![1i64; mask_len]))?.into_dyn().into()));
            let plen = positions.len();
            feed.push(("position_ids".into(), Tensor::from_array(([1usize, plen], positions.clone()))?.into_dyn().into()));
            for (name, v) in past.drain(..) {
                feed.push((name.into(), v.into()));
            }
            let mut out = self.lm.run(feed)?;
            let next = {
                let (shape, logits) = out["logits"].try_extract_tensor::<f32>()?;
                let (s, v) = (shape[1] as usize, shape[2] as usize);
                let mut last = logits[(s - 1) * v..s * v].to_vec();
                let mut seen = generated.clone();
                seen.sort_unstable();
                seen.dedup();
                for id in seen {
                    let x = &mut last[id as usize];
                    *x = if *x < 0.0 { *x * REPETITION_PENALTY } else { *x / REPETITION_PENALTY };
                }
                last.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i as i64).unwrap_or(STOP)
            };
            for l in 0..LAYERS {
                for kv in ["key", "value"] {
                    let v = out
                        .remove(&format!("present.{l}.{kv}"))
                        .ok_or_else(|| anyhow::anyhow!("missing present.{l}.{kv}"))?;
                    past.push((format!("past_key_values.{l}.{kv}"), v));
                }
            }
            drop(out);
            generated.push(next);
            if next == STOP {
                break;
            }
            inputs_embeds = Tensor::from_array(([1usize, 1, HIDDEN], self.embed(vec![next])?))?.into_dyn();
            mask_len += 1;
            positions = vec![positions.last().copied().unwrap_or(0) + 1];
        }
        if !keep_going() {
            return Ok(None);
        }

        let mut tokens = self.voice.prompt_tokens.clone();
        tokens.extend(generated.iter().copied().filter(|&t| t != START && t != STOP));
        tokens.extend_from_slice(&[SILENCE; 3]);
        let n = tokens.len();
        let v = &self.voice;
        let emb = Tensor::from_array((v.speaker_embeddings.0.clone(), v.speaker_embeddings.1.clone()))?;
        let feats = Tensor::from_array((v.speaker_features.0.clone(), v.speaker_features.1.clone()))?;
        let out = self.decoder.run(ort::inputs![
            "speech_tokens" => Tensor::from_array(([1usize, n], tokens))?,
            "speaker_embeddings" => emb,
            "speaker_features" => feats
        ])?;
        let (_, wav) = out[0].try_extract_tensor::<f32>()?;
        Ok(Some(wav.to_vec()))
    }
}

fn parse_conditioning(bytes: &[u8]) -> anyhow::Result<Conditioning> {
    anyhow::ensure!(bytes.starts_with(b"CBT1"), "bad voice file");
    let mut at = 4;
    let mut read_array = || -> anyhow::Result<(u8, Vec<usize>, &[u8])> {
        let dtype = bytes[at];
        let rank = bytes[at + 1] as usize;
        at += 2;
        let mut shape = Vec::with_capacity(rank);
        for _ in 0..rank {
            shape.push(u32::from_le_bytes(bytes[at..at + 4].try_into()?) as usize);
            at += 4;
        }
        let len = shape.iter().product::<usize>() * if dtype == 0 { 4 } else { 8 };
        let data = &bytes[at..at + len];
        at += len;
        Ok((dtype, shape, data))
    };
    let f32s = |d: &[u8]| d.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>();
    let (_, fs, fd) = read_array()?;
    let (_, _, pd) = read_array()?;
    let (_, es, ed) = read_array()?;
    let (_, ss, sd) = read_array()?;
    Ok(Conditioning {
        features: (fs, f32s(fd)),
        prompt_tokens: pd.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect(),
        speaker_embeddings: (es, f32s(ed)),
        speaker_features: (ss, f32s(sd)),
    })
}
