//! Microphone capture on a dedicated thread (cpal streams aren't `Send` everywhere).
//! The stream is opened only while the user holds the talk key, so the OS
//! microphone indicator is on exactly when Blurt is listening.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use parking_lot::Mutex;
use rubato::{FftFixedIn, Resampler};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const TARGET_RATE: u32 = 16_000;

enum Cmd {
    Start,
    Stop(Sender<anyhow::Result<Vec<f32>>>),
    Cancel,
    /// The last N seconds so far, at 16 kHz, without stopping.
    Peek(f32, Sender<Vec<f32>>),
}

#[derive(Clone)]
pub struct Recorder {
    tx: Sender<Cmd>,
}

struct Capture {
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
    _stream: Stream,
}

impl Recorder {
    /// `on_level` receives a 0.0–1.0 loudness roughly every 50 ms while recording.
    pub fn spawn(on_level: impl Fn(f32) + Send + Sync + 'static) -> Self {
        let (tx, rx) = channel::<Cmd>();
        let on_level = Arc::new(on_level);
        std::thread::Builder::new()
            .name("blurt-audio".into())
            .spawn(move || audio_thread(rx, on_level))
            .expect("spawn audio thread");
        Self { tx }
    }

    pub fn start(&self) {
        let _ = self.tx.send(Cmd::Start);
    }

    /// Stops recording and returns 16 kHz mono samples.
    pub fn stop(&self) -> anyhow::Result<Vec<f32>> {
        let (tx, rx) = channel();
        self.tx.send(Cmd::Stop(tx))?;
        rx.recv_timeout(Duration::from_secs(5))?
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(Cmd::Cancel);
    }

    /// The last `seconds` of the current recording at 16 kHz (empty if not recording).
    pub fn peek(&self, seconds: f32) -> Vec<f32> {
        let (tx, rx) = channel();
        if self.tx.send(Cmd::Peek(seconds, tx)).is_err() {
            return Vec::new();
        }
        rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default()
    }
}

fn audio_thread(rx: Receiver<Cmd>, on_level: Arc<dyn Fn(f32) + Send + Sync>) {
    let mut current: Option<Capture> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Start => {
                current = None;
                match open(on_level.clone()) {
                    Ok(c) => current = Some(c),
                    Err(e) => log::error!("microphone: {e:#}"),
                }
            }
            Cmd::Stop(reply) => {
                let result = match current.take() {
                    Some(c) => {
                        drop(c._stream);
                        let raw = std::mem::take(&mut *c.samples.lock());
                        Ok(resample(&raw, c.rate))
                    }
                    None => Err(anyhow::anyhow!("Blurt couldn't open the microphone")),
                };
                let _ = reply.send(result);
            }
            Cmd::Cancel => current = None,
            Cmd::Peek(seconds, reply) => {
                let out = match current.as_ref() {
                    Some(c) => {
                        let buf = c.samples.lock();
                        let n = (seconds * c.rate as f32) as usize;
                        let tail = &buf[buf.len().saturating_sub(n)..];
                        resample(tail, c.rate)
                    }
                    None => Vec::new(),
                };
                let _ = reply.send(out);
            }
        }
    }
}

fn open(on_level: Arc<dyn Fn(f32) + Send + Sync>) -> anyhow::Result<Capture> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("no microphone found"))?;
    let supported = device.default_input_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;

    let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(rate as usize * 30)));
    let sink = samples.clone();
    let mut meter = Meter::new(on_level);
    let err = |e| log::error!("audio stream: {e}");

    // Downmix every frame to mono f32 and feed the level meter.
    macro_rules! stream {
        ($t:ty, $to_f32:expr) => {
            device.build_input_stream(
                &config,
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let mut buf = sink.lock();
                    for frame in data.chunks(channels) {
                        let s = frame.iter().map(|&x| $to_f32(x)).sum::<f32>() / channels as f32;
                        buf.push(s);
                        meter.push(s);
                    }
                },
                err,
                None,
            )?
        };
    }

    let stream = match format {
        SampleFormat::F32 => stream!(f32, |x: f32| x),
        SampleFormat::I16 => stream!(i16, |x: i16| x as f32 / i16::MAX as f32),
        SampleFormat::I32 => stream!(i32, |x: i32| x as f32 / i32::MAX as f32),
        SampleFormat::U16 => stream!(u16, |x: u16| (x as f32 - 32768.0) / 32768.0),
        other => anyhow::bail!("unsupported microphone sample format {other:?}"),
    };
    stream.play()?;
    Ok(Capture {
        samples,
        rate,
        _stream: stream,
    })
}

/// RMS loudness, reported at most every 50 ms.
struct Meter {
    sum: f32,
    n: usize,
    last: Instant,
    cb: Arc<dyn Fn(f32) + Send + Sync>,
}

impl Meter {
    fn new(cb: Arc<dyn Fn(f32) + Send + Sync>) -> Self {
        Self {
            sum: 0.0,
            n: 0,
            last: Instant::now(),
            cb,
        }
    }
    fn push(&mut self, s: f32) {
        self.sum += s * s;
        self.n += 1;
        if self.last.elapsed() >= Duration::from_millis(50) && self.n > 0 {
            let rms = (self.sum / self.n as f32).sqrt();
            // Speech sits around 0.02–0.2 RMS; stretch it onto 0..1 for the UI.
            (self.cb)((rms * 6.0).sqrt().min(1.0));
            self.sum = 0.0;
            self.n = 0;
            self.last = Instant::now();
        }
    }
}

/// Resamples mono audio to 16 kHz, which the speech models expect.
pub fn resample(input: &[f32], from: u32) -> Vec<f32> {
    if from == TARGET_RATE || input.is_empty() {
        return input.to_vec();
    }
    const CHUNK: usize = 1024;
    let mut r = match FftFixedIn::<f32>::new(from as usize, TARGET_RATE as usize, CHUNK, 2, 1) {
        Ok(r) => r,
        Err(e) => {
            log::error!("resampler: {e}");
            return input.to_vec();
        }
    };
    let mut out = Vec::with_capacity(input.len() * TARGET_RATE as usize / from as usize + CHUNK);
    let mut pos = 0;
    while pos + CHUNK <= input.len() {
        if let Ok(o) = r.process(&[&input[pos..pos + CHUNK]], None) {
            out.extend_from_slice(&o[0]);
        }
        pos += CHUNK;
    }
    if pos < input.len() {
        if let Ok(o) = r.process_partial(Some(&[&input[pos..]]), None) {
            out.extend_from_slice(&o[0]);
        }
    }
    // Flush the resampler's internal delay so the last word isn't clipped.
    if let Ok(o) = r.process_partial::<&[f32]>(None, None) {
        out.extend_from_slice(&o[0]);
    }
    out
}
