//! Dev check: how much "shadow" (energy above the original 12 kHz Nyquist) each
//! 24 kHz -> 48 kHz conversion adds. `cargo run --release --example imaging_check -- voice.wav`
use rustfft::{num_complex::Complex, FftPlanner};

fn linear_up(x: &[f32]) -> Vec<f32> {
    // What the audio player does: linear interpolation.
    let mut y = Vec::with_capacity(x.len() * 2);
    for i in 0..x.len() {
        let a = x[i];
        let b = *x.get(i + 1).unwrap_or(&a);
        y.push(a);
        y.push((a + b) / 2.0);
    }
    y
}

fn shadow_db(y: &[f32]) -> f32 {
    let n = 4096;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(n);
    let (mut hi, mut all) = (0f64, 0f64);
    for chunk in y.chunks_exact(n) {
        let mut buf: Vec<Complex<f32>> = chunk
            .iter()
            .enumerate()
            .map(|(i, v)| Complex::new(v * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos()), 0.0))
            .collect();
        fft.process(&mut buf);
        for (k, c) in buf[..n / 2].iter().enumerate() {
            let p = c.norm_sqr() as f64;
            all += p;
            if k as f32 * 48_000.0 / n as f32 > 12_500.0 {
                hi += p;
            }
        }
    }
    (10.0 * (hi / all).log10()) as f32
}

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let mut r = hound::WavReader::open(path).unwrap();
    let x: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
    let lin = linear_up(&x);
    let hq = blurt_lib::voice::polish::resample(&x, 24_000, 48_000);
    println!("shadow above 12.5 kHz, relative to the voice:");
    println!("  audio player (linear):     {:6.1} dB", shadow_db(&lin));
    println!("  Blurt's converter (FFT):   {:6.1} dB", shadow_db(&hq));
    if let Some(dir) = std::env::args().nth(2) {
        let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let after = blurt_lib::voice::polish::sentence(&x, 24_000, 48_000, "done.");
        for (name, y) in [("before.wav", lin), ("after.wav", after)] {
            let mut w = hound::WavWriter::create(format!("{dir}/{name}"), spec).unwrap();
            for s in y { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap(); }
            w.finalize().unwrap();
        }
    }
}
