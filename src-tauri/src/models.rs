//! Downloadable model packs. Everything runs on the user's computer; packs are fetched
//! once into the app data folder.
//!
//! | pack    | what it adds                            | size    | licence                         |
//! |---------|-----------------------------------------|---------|---------------------------------|
//! | speech  | Parakeet TDT 0.6B v3 + Smart Turn v3.2  | ~680 MB | CC-BY-4.0 (NVIDIA) / BSD-2      |
//! | voice   | Pocket TTS int8 (Tally's voice)         | ~98 MB  | CC-BY-4.0 (Kyutai)              |
//! | tone    | emotion2vec+ base (hearing your tone)   | ~373 MB | FunASR model licence (credit)   |
//! | premium | Chatterbox Turbo 4-bit (expressive)     | ~491 MB | MIT (Resemble AI)               |

use futures_util::StreamExt;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pack {
    Speech,
    Voice,
    Tone,
    Premium,
}

struct Source {
    url: &'static str,
    /// Destination relative to the pack folder. For archives, the folder to unpack into.
    dest: &'static str,
    size: u64,
    /// A .tar.bz2 whose single top-level folder is flattened into `dest`.
    archive: bool,
}

fn sources(pack: Pack) -> &'static [Source] {
    match pack {
        Pack::Speech => &[
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/vocab.txt", dest: "vocab.txt", size: 93_939, archive: false },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/config.json", dest: "config.json", size: 97, archive: false },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/nemo128.onnx", dest: "nemo128.onnx", size: 139_764, archive: false },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/decoder_joint-model.int8.onnx", dest: "decoder_joint-model.int8.onnx", size: 18_202_004, archive: false },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/encoder-model.int8.onnx", dest: "encoder-model.int8.onnx", size: 652_183_999, archive: false },
            Source { url: "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/smart-turn-v3.2-cpu.onnx", dest: "smart-turn-v3.2-cpu.onnx", size: 8_700_000, archive: false },
        ],
        Pack::Voice => &[Source {
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-pocket-tts-int8-2026-01-26.tar.bz2",
            dest: "pocket-tts",
            size: 98_336_520,
            archive: true,
        }],
        Pack::Tone => &[
            Source { url: "https://huggingface.co/pankotaro/emotion2vec-plus-base-onnx/resolve/main/emotion2vec_plus_base.onnx", dest: "emotion2vec_plus_base.onnx", size: 373_000_000, archive: false },
            Source { url: "https://huggingface.co/pankotaro/emotion2vec-plus-base-onnx/resolve/main/emotion2vec_head.json", dest: "emotion2vec_head.json", size: 128_000, archive: false },
        ],
        Pack::Premium => &[
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/tokenizer.json", dest: "tokenizer.json", size: 2_100_000, archive: false },
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/onnx/embed_tokens_q4.onnx", dest: "embed_tokens_q4.onnx", size: 100_000, archive: false },
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/onnx/embed_tokens_q4.onnx_data", dest: "embed_tokens_q4.onnx_data", size: 37_000_000, archive: false },
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/onnx/language_model_q4.onnx", dest: "language_model_q4.onnx", size: 500_000, archive: false },
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/onnx/language_model_q4.onnx_data", dest: "language_model_q4.onnx_data", size: 205_000_000, archive: false },
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/onnx/conditional_decoder_q4.onnx", dest: "conditional_decoder_q4.onnx", size: 2_000_000, archive: false },
            Source { url: "https://huggingface.co/ResembleAI/chatterbox-turbo-ONNX/resolve/main/onnx/conditional_decoder_q4.onnx_data", dest: "conditional_decoder_q4.onnx_data", size: 247_000_000, archive: false },
        ],
    }
}

pub fn dir(data_dir: &Path, pack: Pack) -> PathBuf {
    let name = match pack {
        // Kept at the original path so existing installs don't re-download.
        Pack::Speech => "parakeet-tdt-0.6b-v3-int8",
        Pack::Voice => "voice",
        Pack::Tone => "tone",
        Pack::Premium => "premium-voice",
    };
    data_dir.join("models").join(name)
}

pub fn is_ready(data_dir: &Path, pack: Pack) -> bool {
    let d = dir(data_dir, pack);
    sources(pack).iter().all(|s| {
        let p = d.join(s.dest);
        if s.archive { p.is_dir() } else { p.is_file() }
    })
}

pub fn size(pack: Pack) -> u64 {
    sources(pack).iter().map(|s| s.size).sum()
}

/// Downloads whatever is missing from a pack. `progress(done, total)` in bytes.
pub async fn download(data_dir: &Path, pack: Pack, progress: impl Fn(u64, u64)) -> anyhow::Result<()> {
    let d = dir(data_dir, pack);
    tokio::fs::create_dir_all(&d).await?;
    let client = reqwest::Client::builder().user_agent("Blurt (blurt.soonbuilt.com)").build()?;
    let total = size(pack);
    let mut before = 0u64;

    for s in sources(pack) {
        let dest = d.join(s.dest);
        let done = if s.archive { dest.is_dir() } else { dest.is_file() };
        if done {
            before += s.size;
            progress(before, total);
            continue;
        }
        let resp = client.get(s.url).send().await?.error_for_status()?;
        let part = d.join(format!("{}.part", s.dest));
        let mut file = tokio::fs::File::create(&part).await?;
        let mut stream = resp.bytes_stream();
        let mut got = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            got += chunk.len() as u64;
            progress(before + got.min(s.size), total);
        }
        file.flush().await?;
        drop(file);
        if s.archive {
            let (part2, dest2) = (part.clone(), dest.clone());
            tokio::task::spawn_blocking(move || unpack(&part2, &dest2)).await??;
            tokio::fs::remove_file(&part).await?;
        } else {
            tokio::fs::rename(&part, &dest).await?;
        }
        before += s.size;
    }
    progress(total, total);
    Ok(())
}

/// Unpacks a .tar.bz2 with one top-level folder into `dest` (without that folder level).
fn unpack(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let tmp = dest.with_extension("unpacking");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let file = std::fs::File::open(archive)?;
    let mut decoder = bzip2::read::BzDecoder::new(file);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    tar::Archive::new(buf.as_slice()).unpack(&tmp)?;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&tmp)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let inner = if entries.len() == 1 && entries[0].is_dir() { entries.remove(0) } else { tmp.clone() };
    std::fs::rename(&inner, dest)?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}
