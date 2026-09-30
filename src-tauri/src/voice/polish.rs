//! Getting generated speech ready for the speakers.
//!
//! The voices are generated at 24 kHz and most Macs play at 48 kHz. The audio player's own
//! conversion is linear interpolation, which adds a harsh "shadow" copy of the voice above
//! 12 kHz at about -26 dB (heard as a raw, metallic voice behind Tally). We convert with a
//! proper FFT resampler instead (shadow at about -140 dB). Also: near-silence between
//! words is turned down, sentence edges fade to avoid clicks, and each sentence is followed
//! by a short pause so Tally has room to breathe.

use rubato::{FftFixedIn, Resampler};

/// Gate threshold: this far below the sentence's speech level counts as a gap. Kept deep so
/// soft consonants and breaths inside words are never touched, only near-silence.
const GATE_BELOW_SPEECH_DB: f32 = 45.0;
/// Gaps are turned down by this much (not muted, so it never sounds chopped).
const GATE_DEPTH_DB: f32 = 30.0;
const TARGET_PEAK: f32 = 0.85;
const MAX_GAIN: f32 = 2.5;

/// Makes one generated sentence ready to play at `out_rate`, followed by a pause that
/// suits how the sentence ends.
pub fn sentence(samples: &[f32], rate: u32, out_rate: u32, sentence_text: &str) -> Vec<f32> {
    let mut x = trim(samples, rate);
    gate(&mut x, rate);
    normalize(&mut x);
    fade(&mut x, rate, 8.0);
    let mut out = resample(&x, rate, out_rate);
    let pause_ms = match sentence_text.trim_end().chars().last() {
        Some('?') | Some('!') => 380,
        Some('.') | Some(':') | Some(';') => 300,
        _ => 200,
    };
    out.extend(std::iter::repeat(0.0).take(out_rate as usize * pause_ms / 1000));
    out
}

fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

fn rms(frame: &[f32]) -> f32 {
    (frame.iter().map(|v| v * v).sum::<f32>() / frame.len().max(1) as f32).sqrt()
}

/// Drops near-silent lead-in and tail (keeps 40 ms so words aren't clipped).
fn trim(x: &[f32], rate: u32) -> Vec<f32> {
    let frame = (rate as usize / 200).max(1); // 5 ms
    let loud: Vec<bool> = x.chunks(frame).map(|c| db(rms(c)) > -50.0).collect();
    let (Some(first), Some(last)) = (loud.iter().position(|&l| l), loud.iter().rposition(|&l| l)) else {
        return x.to_vec();
    };
    let keep = rate as usize / 25;
    let start = (first * frame).saturating_sub(keep);
    let end = ((last + 1) * frame + keep).min(x.len());
    x[start..end].to_vec()
}

/// Smooth downward gate: gaps between words fade down, speech passes untouched.
pub fn gate(x: &mut [f32], rate: u32) {
    let frame = (rate as usize / 200).max(1); // 5 ms envelope
    let env: Vec<f32> = x.chunks(frame).map(|c| db(rms(c))).collect();
    if env.is_empty() {
        return;
    }
    let mut sorted = env.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let speech = sorted[sorted.len() * 8 / 10];
    let threshold = (speech - GATE_BELOW_SPEECH_DB).max(-66.0);
    let floor = 10f32.powf(-GATE_DEPTH_DB / 20.0);

    // Per-frame target gain with a short hold so word tails aren't cut.
    let hold_frames = 8; // 40 ms
    let mut open_for = 0usize;
    let targets: Vec<f32> = env
        .iter()
        .map(|&e| {
            if e >= threshold {
                open_for = hold_frames;
                1.0
            } else if open_for > 0 {
                open_for -= 1;
                1.0
            } else {
                floor
            }
        })
        .collect();

    // Opens fast (2 ms), closes smoothly (25 ms after a 40 ms hold), so it never clicks.
    let attack = 1.0 - (-1.0 / (0.002 * rate as f32)).exp();
    let release = 1.0 - (-1.0 / (0.025 * rate as f32)).exp();
    let mut g = targets[0];
    for (i, s) in x.iter_mut().enumerate() {
        let t = targets[(i / frame).min(targets.len() - 1)];
        g += (t - g) * if t > g { attack } else { release };
        *s *= g;
    }
}

fn normalize(x: &mut [f32]) {
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    if peak > 1e-4 {
        let gain = (TARGET_PEAK / peak).min(MAX_GAIN);
        x.iter_mut().for_each(|v| *v *= gain);
    }
}

fn fade(x: &mut [f32], rate: u32, ms: f32) {
    let n = ((rate as f32 * ms / 1000.0) as usize).min(x.len() / 2);
    for i in 0..n {
        let g = i as f32 / n as f32;
        x[i] *= g;
        let j = x.len() - 1 - i;
        x[j] *= g;
    }
}

/// High-quality sample-rate conversion (the audio player's built-in one is basic).
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    const CHUNK: usize = 1024;
    let Ok(mut r) = FftFixedIn::<f32>::new(from as usize, to as usize, CHUNK, 2, 1) else {
        return x.to_vec();
    };
    let mut out = Vec::with_capacity(x.len() * to as usize / from as usize + CHUNK);
    let mut pos = 0;
    while pos + CHUNK <= x.len() {
        if let Ok(o) = r.process(&[&x[pos..pos + CHUNK]], None) {
            out.extend_from_slice(&o[0]);
        }
        pos += CHUNK;
    }
    if pos < x.len() {
        if let Ok(o) = r.process_partial(Some(&[&x[pos..]]), None) {
            out.extend_from_slice(&o[0]);
        }
    }
    if let Ok(o) = r.process_partial::<&[f32]>(None, None) {
        out.extend_from_slice(&o[0]);
    }
    // The resampler delays its output a little; drop that lead-in so timing stays tight.
    let delay = r.output_delay();
    if out.len() > delay {
        out.drain(..delay);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_quietens_gaps_not_speech() {
        let rate = 24_000;
        // 0.3 s of "speech" (loud tone), 0.3 s of residue (-62 dB noise-like tone), speech again.
        let tone = |amp: f32, n: usize| (0..n).map(move |i| amp * (i as f32 * 0.07).sin());
        let mut x: Vec<f32> = tone(0.3, 7200).chain(tone(0.0003, 7200)).chain(tone(0.3, 7200)).collect();
        gate(&mut x, rate);
        let speech = rms(&x[1000..6000]);
        let gap = rms(&x[11_000..14_000]);
        assert!(db(speech) > -14.0, "speech kept: {}", db(speech)); // a 0.3 sine is -13.5 dB
        assert!(db(gap) < -95.0, "gap quietened: {}", db(gap)); // was -73 dB
    }
}
