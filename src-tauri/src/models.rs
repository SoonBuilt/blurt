//! Downloadable model packs. Everything runs on the user's computer; packs are fetched
//! once into the app data folder.
//!
//! | pack    | what it adds                            | size    | licence                         |
//! |---------|-----------------------------------------|---------|---------------------------------|
//! | speech  | Parakeet TDT 0.6B v3 + Smart Turn v3.2  | ~680 MB | CC-BY-4.0 (NVIDIA) / BSD-2      |
//! | tone    | emotion2vec+ base (hearing your tone)   | ~373 MB | FunASR model licence (credit)   |

use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pack {
    Speech,
    Tone,
}

struct Source {
    url: &'static str,
    /// Destination relative to the pack folder.
    dest: &'static str,
    size: u64,
}

fn sources(pack: Pack) -> &'static [Source] {
    match pack {
        Pack::Speech => &[
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/vocab.txt", dest: "vocab.txt", size: 93_939 },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/config.json", dest: "config.json", size: 97 },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/nemo128.onnx", dest: "nemo128.onnx", size: 139_764 },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/decoder_joint-model.int8.onnx", dest: "decoder_joint-model.int8.onnx", size: 18_202_004 },
            Source { url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/encoder-model.int8.onnx", dest: "encoder-model.int8.onnx", size: 652_183_999 },
            Source { url: "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/smart-turn-v3.2-cpu.onnx", dest: "smart-turn-v3.2-cpu.onnx", size: 8_700_000 },
        ],
        Pack::Tone => &[
            Source { url: "https://huggingface.co/pankotaro/emotion2vec-plus-base-onnx/resolve/main/emotion2vec_plus_base.onnx", dest: "emotion2vec_plus_base.onnx", size: 373_000_000 },
            Source { url: "https://huggingface.co/pankotaro/emotion2vec-plus-base-onnx/resolve/main/emotion2vec_head.json", dest: "emotion2vec_head.json", size: 128_000 },
        ],
    }
}

pub fn dir(data_dir: &Path, pack: Pack) -> PathBuf {
    let name = match pack {
        // Kept at the original path so existing installs don't re-download.
        Pack::Speech => "parakeet-tdt-0.6b-v3-int8",
        Pack::Tone => "tone",
    };
    data_dir.join("models").join(name)
}

pub fn is_ready(data_dir: &Path, pack: Pack) -> bool {
    let d = dir(data_dir, pack);
    sources(pack).iter().all(|s| d.join(s.dest).is_file())
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
        if dest.is_file() {
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
        tokio::fs::rename(&part, &dest).await?;
        before += s.size;
    }
    progress(total, total);
    Ok(())
}
