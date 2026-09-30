//! Whisper log-mel features for Smart Turn, matching Hugging Face's WhisperFeatureExtractor
//! (n_fft 400, hop 160, 80 Slaney mel bins, log10, 8 dB dynamic range, (x+4)/4).

use rustfft::{num_complex::Complex, FftPlanner};

const N_FFT: usize = 400;
const HOP: usize = 160;
const N_MELS: usize = 80;
const SR: f32 = 16000.0;

fn hz_to_mel(f: f32) -> f32 {
    let (f_sp, min_log_hz) = (200.0 / 3.0, 1000.0);
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f32).ln() / 27.0;
    if f >= min_log_hz { min_log_mel + (f / min_log_hz).ln() / logstep } else { f / f_sp }
}
fn mel_to_hz(m: f32) -> f32 {
    let (f_sp, min_log_hz) = (200.0 / 3.0, 1000.0);
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f32).ln() / 27.0;
    if m >= min_log_mel { min_log_hz * (logstep * (m - min_log_mel)).exp() } else { f_sp * m }
}

fn filterbank() -> Vec<Vec<f32>> {
    let n_freq = N_FFT / 2 + 1;
    let fft_freqs: Vec<f32> = (0..n_freq).map(|i| i as f32 * (SR / 2.0) / (n_freq - 1) as f32).collect();
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(SR / 2.0));
    let pts: Vec<f32> = (0..N_MELS + 2).map(|i| mel_to_hz(lo + (hi - lo) * i as f32 / (N_MELS + 1) as f32)).collect();
    (0..N_MELS)
        .map(|m| {
            let (l, c, r) = (pts[m], pts[m + 1], pts[m + 2]);
            let enorm = 2.0 / (r - l);
            fft_freqs
                .iter()
                .map(|&f| {
                    let down = (f - l) / (c - l);
                    let up = (r - f) / (r - c);
                    down.min(up).max(0.0) * enorm
                })
                .collect()
        })
        .collect()
}

/// `audio` must be exactly 8 s at 16 kHz. Returns [80 * 800] row-major (mel, frame).
pub fn whisper_features(audio: &[f32]) -> Vec<f32> {
    // Zero-mean, unit-variance normalisation over the whole window.
    let n = audio.len() as f32;
    let mean = audio.iter().sum::<f32>() / n;
    let var = audio.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    let x: Vec<f32> = audio.iter().map(|v| (v - mean) / (var + 1e-7).sqrt()).collect();

    // centre=True with reflect padding
    let pad = N_FFT / 2;
    let mut padded = Vec::with_capacity(x.len() + 2 * pad);
    for i in (1..=pad).rev() { padded.push(x[i]); }
    padded.extend_from_slice(&x);
    for i in 0..pad { padded.push(x[x.len() - 2 - i]); }

    let window: Vec<f32> = (0..N_FFT).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N_FFT as f32).cos()).collect();
    let fft = FftPlanner::<f32>::new().plan_fft_forward(N_FFT);
    let n_frames = 1 + (padded.len() - N_FFT) / HOP;
    let frames = n_frames - 1; // Whisper drops the last frame
    let fb = filterbank();
    let mut out = vec![0f32; N_MELS * frames];
    let mut buf = vec![Complex::new(0f32, 0f32); N_FFT];
    for t in 0..frames {
        for i in 0..N_FFT { buf[i] = Complex::new(padded[t * HOP + i] * window[i], 0.0); }
        fft.process(&mut buf);
        let power: Vec<f32> = buf[..N_FFT / 2 + 1].iter().map(|c| c.norm_sqr()).collect();
        for m in 0..N_MELS {
            let e: f32 = fb[m].iter().zip(&power).map(|(w, p)| w * p).sum();
            out[m * frames + t] = e.max(1e-10).log10();
        }
    }
    let max = out.iter().cloned().fold(f32::MIN, f32::max);
    for v in out.iter_mut() { *v = (v.max(max - 8.0) + 4.0) / 4.0; }
    out
}
