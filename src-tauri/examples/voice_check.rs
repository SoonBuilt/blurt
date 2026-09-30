//! Dev check for the voice layer with the app's real code:
//! `cargo run --release --example voice_check -- <data-dir> <angry.wav> <finished.wav>`
use blurt_lib::voice::{speaker::Speaker, tone::ToneDetector, turn};
use std::path::PathBuf;
use std::time::Instant;

fn read(path: &str) -> Vec<f32> {
    let mut r = hound::WavReader::open(path).unwrap();
    let spec = r.spec();
    let s: Vec<f32> = r.samples::<i16>().map(|x| x.unwrap() as f32 / 32768.0).collect();
    let mono: Vec<f32> = s.chunks(spec.channels as usize).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect();
    blurt_lib::audio::resample(&mono, spec.sample_rate)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = PathBuf::from(&a[1]);
    let tone = ToneDetector::new();
    let t = Instant::now();
    println!("tone of angry clip: {:?} ({:?})", tone.detect(&data, &read(&a[2])), t.elapsed());
    let finished = read(&a[3]);
    let td = turn::TurnDetector::new();
    println!("finished sentence -> done p = {:.2}", td.done_probability(&data, &finished).unwrap());
    println!("cut mid-sentence  -> done p = {:.2}", td.done_probability(&data, &finished[..16000 * 4]).unwrap());
    let ep = turn::endpoint(&finished);
    println!("endpoint: heard_speech={} trailing silence={:.2}s", ep.heard_speech, ep.silence);
    let sp = Speaker::spawn(data.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    let t = Instant::now();
    sp.say("Hi, I'm Tally! Press Command Shift 4, then Space, then click the window.", move || { let _ = tx.send(()); });
    rx.recv().unwrap();
    println!("spoke in {:?}", t.elapsed());
}
