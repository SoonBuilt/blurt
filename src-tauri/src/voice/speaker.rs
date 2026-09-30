//! Tally's voice: Kokoro's af_heart (Kokoro-82M, Apache-2.0), slowed a touch for a warm,
//! unhurried pace. Speaks sentence by sentence, so the first words play while the rest is
//! still being generated. Any key press (or Esc) interrupts it. The Pro voice (Chatterbox,
//! see `premium`) is conditioned on the same Heart voice so Tally sounds the same.

use crate::models::{self, Pack};
use parking_lot::Mutex;
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamBuilder, Sink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::Duration;

/// Kokoro speaker id of af_heart in the multi-lang v1.0 voice table.
const HEART: i32 = 3;
/// >1 slows speech down; 1.12 gives Tally a relaxed, easygoing pace.
const PACE: f32 = 1.12;

enum Cmd {
    Speak { text: String, gen: u64, done: Box<dyn FnOnce() + Send> },
    Warm,
}

#[derive(Clone)]
pub struct Speaker {
    tx: Sender<Cmd>,
    gen: Arc<AtomicU64>,
    premium: Arc<AtomicBool>,
    speaking: Arc<AtomicBool>,
    sink: Arc<Mutex<Option<Arc<Sink>>>>,
}

impl Speaker {
    pub fn spawn(data_dir: PathBuf) -> Self {
        let (tx, rx) = channel::<Cmd>();
        let gen = Arc::new(AtomicU64::new(0));
        let speaking = Arc::new(AtomicBool::new(false));
        let sink: Arc<Mutex<Option<Arc<Sink>>>> = Arc::new(Mutex::new(None));
        let premium = Arc::new(AtomicBool::new(false));
        let (g, sp, sk, pr) = (gen.clone(), speaking.clone(), sink.clone(), premium.clone());
        std::thread::Builder::new()
            .name("blurt-voice".into())
            .spawn(move || {
                let mut engine: Option<sherpa_onnx::OfflineTts> = None;
                let mut expressive: Option<super::premium::Premium> = None;
                let mut stream: Option<OutputStream> = None;
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        Cmd::Warm => {
                            if pr.load(Ordering::SeqCst) && super::premium::Premium::is_ready(&data_dir) {
                                if expressive.is_none() {
                                    expressive = super::premium::Premium::load(&data_dir).map_err(|e| log::error!("premium voice: {e:#}")).ok();
                                }
                            } else if Speaker::is_ready(&data_dir) {
                                let _ = load(&mut engine, &data_dir);
                            }
                        }
                        Cmd::Speak { text, gen: my_gen, done } => {
                            if g.load(Ordering::SeqCst) != my_gen {
                                done();
                                continue;
                            }
                            let use_premium = pr.load(Ordering::SeqCst) && super::premium::Premium::is_ready(&data_dir);
                            if use_premium && expressive.is_none() {
                                expressive = super::premium::Premium::load(&data_dir).map_err(|e| log::error!("premium voice: {e:#}")).ok();
                            }
                            let voice = match (use_premium, expressive.as_mut()) {
                                (true, Some(p)) => Voice::Premium(p),
                                _ => Voice::Free(&mut engine),
                            };
                            if let Err(e) = speak(&text, my_gen, &g, voice, &mut stream, &sk, &data_dir) {
                                log::error!("speaking: {e:#}");
                            }
                            sp.store(false, Ordering::SeqCst);
                            done();
                        }
                    }
                }
            })
            .expect("spawn voice thread");
        Self { tx, gen, premium, speaking, sink }
    }

    /// Switches between the free voice and the premium (Chatterbox) voice.
    pub fn set_premium(&self, on: bool) {
        self.premium.store(on, Ordering::SeqCst);
    }

    /// True when any voice is installed.
    pub fn is_ready(data_dir: &std::path::Path) -> bool {
        models::is_ready(data_dir, Pack::Voice) || super::premium::Premium::is_ready(data_dir)
    }

    /// Loads the voice model in the background so the first answer starts quickly.
    pub fn warm(&self) {
        let _ = self.tx.send(Cmd::Warm);
    }

    /// Speaks `text`; `done` runs when Tally finishes or is interrupted.
    pub fn say(&self, text: &str, done: impl FnOnce() + Send + 'static) {
        self.stop();
        let gen = self.gen.load(Ordering::SeqCst);
        self.speaking.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Cmd::Speak { text: text.to_string(), gen, done: Box::new(done) });
    }

    pub fn is_speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }

    /// Stops talking immediately (barge-in).
    pub fn stop(&self) {
        self.gen.fetch_add(1, Ordering::SeqCst);
        if let Some(s) = self.sink.lock().as_ref() {
            s.stop();
        }
    }
}

fn load<'a>(engine: &'a mut Option<sherpa_onnx::OfflineTts>, data_dir: &std::path::Path) -> anyhow::Result<&'a sherpa_onnx::OfflineTts> {
    if engine.is_none() {
        let d = models::dir(data_dir, Pack::Voice).join("kokoro");
        let p = |f: &str| Some(d.join(f).to_string_lossy().into_owned());
        let mut cfg = sherpa_onnx::OfflineTtsConfig::default();
        cfg.model.kokoro.model = p("model.int8.onnx");
        cfg.model.kokoro.voices = p("voices.bin");
        cfg.model.kokoro.tokens = p("tokens.txt");
        cfg.model.kokoro.data_dir = p("espeak-ng-data");
        cfg.model.kokoro.lexicon = p("lexicon-us-en.txt");
        cfg.model.kokoro.length_scale = PACE;
        cfg.model.num_threads = std::thread::available_parallelism().map(|n| n.get().clamp(2, 4) as i32).unwrap_or(2);
        *engine = Some(sherpa_onnx::OfflineTts::create(&cfg).ok_or_else(|| anyhow::anyhow!("couldn't load Tally's voice"))?);
    }
    Ok(engine.as_ref().unwrap())
}

enum Voice<'a> {
    Free(&'a mut Option<sherpa_onnx::OfflineTts>),
    Premium(&'a mut super::premium::Premium),
}

#[allow(clippy::too_many_arguments)]
fn speak(
    text: &str,
    my_gen: u64,
    gen: &Arc<AtomicU64>,
    mut voice: Voice,
    stream: &mut Option<OutputStream>,
    sink_slot: &Arc<Mutex<Option<Arc<Sink>>>>,
    data_dir: &std::path::Path,
) -> anyhow::Result<()> {
    if stream.is_none() {
        let mut s = OutputStreamBuilder::open_default_stream()?;
        s.log_on_drop(false);
        *stream = Some(s);
    }
    // A fresh sink per answer: a stopped sink can't be reused reliably.
    let out_rate = stream.as_ref().unwrap().config().sample_rate();
    let sink = Arc::new(Sink::connect_new(stream.as_ref().unwrap().mixer()));
    *sink_slot.lock() = Some(sink.clone());

    let alive = |g: &Arc<AtomicU64>| g.load(Ordering::SeqCst) == my_gen;
    let premium = matches!(voice, Voice::Premium(_));
    // Only the premium voice can perform [laugh]-style tags; the free one would read them out.
    let script = if premium { clean_for_speech(text) } else { strip_tags(&clean_for_speech(text)) };
    for sentence in sentences(&script) {
        if !alive(gen) {
            return Ok(());
        }
        let (samples, rate) = match &mut voice {
            Voice::Premium(p) => {
                let g2 = gen.clone();
                match p.generate(&sentence, move || g2.load(Ordering::SeqCst) == my_gen)? {
                    Some(wav) => (wav, super::premium::SAMPLE_RATE),
                    None => return Ok(()),
                }
            }
            Voice::Free(engine) => {
                let tts = load(engine, data_dir)?;
                let g2 = gen.clone();
                let cfg = sherpa_onnx::GenerationConfig { sid: HEART, speed: 1.0, ..Default::default() };
                // Returning false from the callback aborts generation when interrupted.
                let audio = tts.generate_with_config(&sentence, &cfg, Some(move |_: &[f32], _: f32| g2.load(Ordering::SeqCst) == my_gen));
                let Some(audio) = audio else { continue };
                (audio.samples().to_vec(), audio.sample_rate() as u32)
            }
        };
        if !alive(gen) {
            return Ok(());
        }
        // Clean up the gaps, fade the edges, add a breath, and convert for the speakers.
        let ready = super::polish::sentence(&samples, rate, out_rate, &sentence);
        sink.append(SamplesBuffer::new(1, out_rate, ready));
    }
    while !sink.empty() && alive(gen) {
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}

/// Removes markdown and symbols that sound silly read aloud.
fn clean_for_speech(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let l = line.trim().trim_start_matches(['#', '-', '*', '•', '>', ' ']).replace(['*', '_', '`', '#'], "");
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        if !out.is_empty() {
            // Each line becomes its own sentence.
            if !out.ends_with(['.', '!', '?', ':', ';']) {
                out.push('.');
            }
            out.push(' ');
        }
        out.push_str(l);
    }
    out
}

/// Removes [laugh]-style performance tags (for display, and for the free voice).
pub fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Splits into sentences, merging very short ones so the voice keeps its flow.
fn sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        cur.push(c);
        let end = matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if end && cur.trim().len() >= 24 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        match out.last_mut() {
            Some(last) if cur.trim().len() < 24 => {
                last.push(' ');
                last.push_str(cur.trim());
            }
            _ => out.push(cur.trim().to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        let s = sentences("Press Command Shift 4. Then Space. Then click the window you want to capture.");
        assert_eq!(s, vec!["Press Command Shift 4. Then Space.", "Then click the window you want to capture."]);
    }

    #[test]
    fn strips_tags() {
        assert_eq!(strip_tags("Ha! [laugh] That's great. [sigh]"), "Ha! That's great.");
    }

    #[test]
    fn strips_markdown() {
        assert_eq!(clean_for_speech("**Yes!**\n- one\n- two"), "Yes! one. two");
    }
}
