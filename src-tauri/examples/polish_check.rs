//! Dev check: run a generated clip through the playback clean-up.
//! `cargo run --release --example polish_check -- in.wav out.wav`
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut r = hound::WavReader::open(&a[1]).unwrap();
    let rate = r.spec().sample_rate;
    let x: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
    let y = blurt_lib::voice::polish::sentence(&x, rate, 48_000, "done.");
    let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&a[2], spec).unwrap();
    for s in y { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap(); }
    w.finalize().unwrap();
}
