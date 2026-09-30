//! The coordinator: key presses in, text (or Tally's voice) out.
//!
//! hold key → record → release → transcribe (+ hear the tone) → dictate: clean up and paste
//! | ask AI: run the prompt with any highlighted text as context → paste it, or say it aloud.
//! Hands-free: double-tap, talk, and Smart Turn decides when you've finished.

use crate::hud::{self, HudState};
use crate::keys::{KeyAction, Mode};
use crate::models::{self, Pack};
use crate::settings::{DictationStyle, Settings};
use crate::tray::{self, Mood};
use crate::voice::{speaker::Speaker, tone::ToneDetector, turn};
use crate::{ai, audio::Recorder, insert, stt::Transcriber, text};
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// Recordings shorter than this are treated as accidental presses.
const MIN_SAMPLES: usize = crate::audio::TARGET_RATE as usize * 3 / 10;
/// Hands-free safety limits.
const HANDS_FREE_MAX: Duration = Duration::from_secs(90);
const HANDS_FREE_GIVE_UP_SILENCE: f32 = 2.5;

pub struct Engine {
    pub app: AppHandle,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub stt: Transcriber,
    pub speaker: Speaker,
    tone: ToneDetector,
    turn: turn::TurnDetector,
    recorder: Recorder,
    mode: Mutex<Mode>,
    recording: AtomicBool,
    busy: AtomicBool,
    /// Bumped on every new recording, so a stale hands-free watcher knows to quit.
    session: AtomicU64,
}

impl Engine {
    pub fn new(app: AppHandle, data_dir: PathBuf, config_dir: PathBuf) -> Arc<Self> {
        let settings = crate::settings::load(&config_dir);
        let level_app = app.clone();
        let recorder = Recorder::spawn(move |l| hud::level(&level_app, l));
        let speaker = Speaker::spawn(data_dir.clone());
        speaker.set_premium(settings.voice.premium_voice);
        Arc::new(Self {
            app,
            data_dir,
            config_dir,
            settings: RwLock::new(settings),
            stt: Transcriber::new(),
            speaker,
            tone: ToneDetector::new(),
            turn: turn::TurnDetector::new(),
            recorder,
            mode: Mutex::new(Mode::Dictate),
            recording: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            session: AtomicU64::new(0),
        })
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    /// Loads optional models in the background so the first use is instant.
    pub fn warm_up(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            if models::is_ready(&me.data_dir, Pack::Speech) {
                if let Err(e) = me.stt.load(&me.data_dir) {
                    log::error!("{e:#}");
                }
            }
            if Speaker::is_ready(&me.data_dir) {
                me.speaker.warm();
            }
            if me.settings.read().voice.tone_awareness {
                me.tone.warm(&me.data_dir);
            }
        });
    }

    pub fn handle(self: &Arc<Self>, action: KeyAction) {
        match action {
            KeyAction::Start(mode) => self.start(mode, false),
            KeyAction::StartHandsFree(mode) => self.start(mode, true),
            KeyAction::SwitchToAi => {
                if self.is_recording() {
                    *self.mode.lock() = Mode::Ai;
                    hud::set(&self.app, HudState::Listening { ai: true, context_words: 0, hands_free: false });
                }
            }
            KeyAction::Cancel => {
                if self.recording.swap(false, Ordering::SeqCst) {
                    self.recorder.cancel();
                    hud::set(&self.app, HudState::Hidden);
                    tray::set_mood(&self.app, Mood::Hello);
                } else if self.speaker.is_speaking() {
                    self.speaker.stop();
                }
            }
            KeyAction::Stop => self.stop_recording(),
        }
    }

    fn stop_recording(self: &Arc<Self>) {
        if self.recording.swap(false, Ordering::SeqCst) {
            let me = self.clone();
            std::thread::spawn(move || me.finish());
        }
    }

    fn start(self: &Arc<Self>, mode: Mode, hands_free: bool) {
        // Talking over Tally interrupts it.
        self.speaker.stop();
        if self.busy.load(Ordering::SeqCst) {
            return; // still writing the last one
        }
        if !models::is_ready(&self.data_dir, Pack::Speech) {
            self.flash_error("Finish setting up Blurt first: it needs its voice model.");
            tray::show_main(&self.app);
            return;
        }
        *self.mode.lock() = mode;
        self.recording.store(true, Ordering::SeqCst);
        let session = self.session.fetch_add(1, Ordering::SeqCst) + 1;
        self.recorder.start();
        tray::set_mood(&self.app, Mood::Listening);
        hud::set(&self.app, HudState::Listening { ai: mode == Mode::Ai, context_words: 0, hands_free });
        if hands_free {
            let me = self.clone();
            std::thread::spawn(move || me.watch_hands_free(session));
        }
    }

    /// Hands-free: after a pause, ask Smart Turn whether the user has finished.
    fn watch_hands_free(self: &Arc<Self>, session: u64) {
        let started = Instant::now();
        let alive = |me: &Arc<Self>| me.is_recording() && me.session.load(Ordering::SeqCst) == session;
        let smart = turn::TurnDetector::is_ready(&self.data_dir);
        let mut last_check_at_silence = 0.0f32;
        while alive(self) {
            std::thread::sleep(Duration::from_millis(200));
            if !alive(self) {
                return;
            }
            if started.elapsed() > HANDS_FREE_MAX {
                break;
            }
            let audio = self.recorder.peek(8.0);
            let ep = turn::endpoint(&audio);
            if !ep.heard_speech {
                // Nothing said for a while after double-tapping: give up quietly.
                if started.elapsed() > Duration::from_secs(8) {
                    self.handle(KeyAction::Cancel);
                    return;
                }
                continue;
            }
            if ep.silence >= HANDS_FREE_GIVE_UP_SILENCE {
                break;
            }
            // A new pause of at least 0.45 s: is it the end of the thought?
            if ep.silence >= 0.45 && (ep.silence - last_check_at_silence).abs() > 0.3 {
                last_check_at_silence = ep.silence;
                if !smart {
                    if ep.silence >= 1.2 {
                        break;
                    }
                    continue;
                }
                match self.turn.done_probability(&self.data_dir, &audio) {
                    Ok(p) if p >= 0.5 => break,
                    Ok(p) => log::debug!("turn: still talking ({p:.2})"),
                    Err(e) => {
                        log::error!("turn detector: {e:#}");
                        if ep.silence >= 1.2 {
                            break;
                        }
                    }
                }
            } else if ep.silence < 0.2 {
                last_check_at_silence = 0.0;
            }
        }
        if alive(self) {
            self.stop_recording();
        }
    }

    fn finish(self: &Arc<Self>) {
        self.busy.store(true, Ordering::SeqCst);
        let result = self.process();
        self.busy.store(false, Ordering::SeqCst);
        match result {
            Ok(Outcome::Done(message)) => {
                tray::set_mood(&self.app, Mood::Ready);
                hud::set(&self.app, HudState::Done { message });
                std::thread::sleep(Duration::from_millis(1100));
                if !self.speaker.is_speaking() && !self.is_recording() {
                    hud::set(&self.app, HudState::Hidden);
                    tray::set_mood(&self.app, Mood::Hello);
                }
            }
            Ok(Outcome::Speaking) => {}
            Ok(Outcome::Nothing) => {
                hud::set(&self.app, HudState::Hidden);
                tray::set_mood(&self.app, Mood::Hello);
            }
            Err(e) => {
                log::error!("{e:#}");
                self.flash_error(&e.to_string());
            }
        }
    }

    fn process(self: &Arc<Self>) -> anyhow::Result<Outcome> {
        let mode = *self.mode.lock();
        let ai_mode = mode == Mode::Ai;
        let samples = Arc::new(self.recorder.stop()?);
        if samples.len() < MIN_SAMPLES {
            return Ok(Outcome::Nothing);
        }
        let settings = self.settings.read().clone();

        tray::set_mood(&self.app, Mood::Working);
        hud::set(&self.app, HudState::Transcribing { ai: ai_mode });

        // Hear the tone while transcribing (only Ask AI uses it).
        let tone_job = (ai_mode && settings.voice.tone_awareness).then(|| {
            let (me, s) = (self.clone(), samples.clone());
            std::thread::spawn(move || me.tone.detect(&me.data_dir, &s))
        });
        let raw = self.stt.transcribe(&self.data_dir, &samples)?;
        let tone = tone_job.and_then(|j| j.join().ok().flatten());
        if raw.trim().is_empty() {
            anyhow::bail!("Tally didn't catch that. Try again a little closer to the mic?");
        }

        if !ai_mode {
            let output = match settings.dictation_style {
                DictationStyle::Verbatim => raw,
                DictationStyle::Clean => text::clean(&raw),
                DictationStyle::Polished => {
                    let cleaned = text::clean(&raw);
                    tauri::async_runtime::block_on(ai::polish(&settings.ai, &cleaned)).unwrap_or(cleaned)
                }
            };
            insert::paste(&self.app, &output, settings.restore_clipboard)?;
            let words = output.split_whitespace().count();
            return Ok(Outcome::Done(format!("{words} word{}", if words == 1 { "" } else { "s" })));
        }

        let instruction = text::clean(&raw);
        // The highlight is still in place: read it now, and pasting replaces it.
        let context = insert::selected_text(&self.app);
        let words = context.as_deref().map(|c| c.split_whitespace().count()).unwrap_or(0);
        hud::set(&self.app, HudState::Thinking { instruction: instruction.clone(), context_words: words, tone });
        let expressive = settings.voice.premium_voice && crate::voice::premium::Premium::is_ready(&self.data_dir);
        let reply = tauri::async_runtime::block_on(ai::ask(&settings.ai, &instruction, context.as_deref(), tone, expressive))?;

        let can_speak = settings.voice.read_answers && Speaker::is_ready(&self.data_dir);
        if reply.spoken && can_speak {
            self.say(&reply.text);
            return Ok(Outcome::Speaking);
        }
        insert::paste(&self.app, &reply.text, settings.restore_clipboard)?;
        if settings.voice.read_everything && can_speak {
            self.say(&reply.text);
            return Ok(Outcome::Speaking);
        }
        Ok(Outcome::Done("Done".into()))
    }

    /// Tally says `text` out loud, showing it in the listening bar while it talks.
    fn say(self: &Arc<Self>, text: &str) {
        tray::set_mood(&self.app, Mood::Ready);
        hud::set(&self.app, HudState::Speaking { text: crate::voice::speaker::strip_tags(text) });
        let me = self.clone();
        self.speaker.say(text, move || {
            // Leave the words up a moment after the voice ends, unless something new started.
            std::thread::sleep(Duration::from_millis(600));
            if !me.is_recording() && !me.busy.load(Ordering::SeqCst) && !me.speaker.is_speaking() {
                hud::set(&me.app, HudState::Hidden);
                tray::set_mood(&me.app, Mood::Hello);
            }
        });
    }

    fn flash_error(&self, message: &str) {
        tray::set_mood(&self.app, Mood::Needs);
        hud::set(&self.app, HudState::Error { message: message.to_string() });
        let app = self.app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(3200));
            hud::set(&app, HudState::Hidden);
            tray::set_mood(&app, Mood::Hello);
        });
    }
}

enum Outcome {
    Done(String),
    Speaking,
    Nothing,
}
