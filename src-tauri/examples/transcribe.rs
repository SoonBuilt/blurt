//! Dev check for the speech pipeline without the UI:
//! `cargo run --release --example transcribe -- <data-dir> <file.wav>`
//! Downloads the model into <data-dir> if needed, then prints raw and cleaned text.

use std::path::PathBuf;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data_dir = PathBuf::from(&args[1]);
    let wav = PathBuf::from(&args[2]);

    if !blurt_lib::stt::is_downloaded(&data_dir) {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(blurt_lib::stt::download(&data_dir, |d, t| {
            eprint!("\rdownloading {:>3}%", d * 100 / t.max(1));
        }))?;
        eprintln!();
    }

    let mut reader = hound::WavReader::open(&wav)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / max)
                .collect()
        }
    };
    let mono: Vec<f32> = samples
        .chunks(spec.channels as usize)
        .map(|f| f.iter().sum::<f32>() / f.len() as f32)
        .collect();
    let audio = blurt_lib::audio::resample(&mono, spec.sample_rate);

    let stt = blurt_lib::stt::Transcriber::new();
    let t = Instant::now();
    stt.load(&data_dir)?;
    eprintln!("model loaded in {:?}", t.elapsed());
    let t = Instant::now();
    let raw = stt.transcribe(&data_dir, &audio)?;
    eprintln!(
        "transcribed {:.1}s of audio in {:?}",
        audio.len() as f32 / 16000.0,
        t.elapsed()
    );
    println!("RAW:   {raw}");
    println!("CLEAN: {}", blurt_lib::text::clean(&raw));
    Ok(())
}
