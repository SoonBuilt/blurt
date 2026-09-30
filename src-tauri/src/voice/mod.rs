//! Blurt's voice layer, all on-device:
//! - `speaker`: Tally reads answers aloud (Kokoro af_heart, Apache-2.0; or the premium
//!   Chatterbox Turbo voice, Resemble AI, MIT, in `premium`)
//! - `tone`: hears how the user sounds (emotion2vec+, FunASR model licence)
//! - `turn`: knows when the user has finished talking (Smart Turn v3.2, BSD-2)

mod mel;
pub mod polish;
pub mod premium;
pub mod speaker;
pub mod tone;
pub mod turn;

/// Opens an ONNX model on the shared runtime with a fixed thread count.
pub(crate) fn onnx_session(path: &std::path::Path, threads: usize) -> anyhow::Result<ort::session::Session> {
    let err = |e: ort::Error<ort::session::builder::SessionBuilder>| anyhow::anyhow!("{e}");
    Ok(ort::session::Session::builder()?
        .with_intra_threads(threads)
        .map_err(err)?
        .commit_from_file(path)?)
}
