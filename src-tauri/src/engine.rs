//! The coordinator: key presses in, text out.
//!
//! hold key → record → release → transcribe (+ hear the tone) → dictate: clean up and paste
//! | ask AI: run the prompt with any highlighted text as context and a short-term memory of
//! the last few exchanges → paste it and leave it on the clipboard.
//! Hands-free: double-tap, talk, and Smart Turn decides when you've finished.

use crate::hud::{self, HudState};
use crate::keys::{KeyAction, Mode};
use crate::models::{self, Pack};
use crate::settings::{DictationStyle, Settings};
use crate::tray::{self, Mood};
use crate::history::{self, History};
use crate::memory::Memory;
use crate::voice::{tone::{Tone, ToneDetector}, turn};
use crate::{ai, audio::Recorder, insert, insert::{Delivered, WhyCopied}, stt::Transcriber, text};
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
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
    pub memory: Memory,
    pub history: History,
    /// The last few tones Tally heard, newest last (for the Smarts page).
    pub recent_tones: Mutex<VecDeque<(Tone, Instant)>>,
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
        let history = History::load(&config_dir);
        Arc::new(Self {
            app,
            data_dir,
            config_dir,
            settings: RwLock::new(settings),
            stt: Transcriber::new(),
            memory: Memory::new(),
            history,
            recent_tones: Mutex::new(VecDeque::new()),
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
            Ok(Outcome::Done(message, tone)) => {
                tray::set_mood(&self.app, Mood::Ready);
                // The "copied, press ⌘V" note needs time to be read; a tone reading a moment too.
                let linger = if message.starts_with("Copied") { 3500 } else if tone.is_some() { 1800 } else { 1100 };
                hud::set(&self.app, HudState::Done { message, tone });
                std::thread::sleep(Duration::from_millis(linger));
                if !self.is_recording() {
                    hud::set(&self.app, HudState::Hidden);
                    tray::set_mood(&self.app, Mood::Hello);
                }
            }
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

        // Hear the tone while transcribing: shown to the user, and Ask AI adapts to it.
        let tone_job = (settings.voice.tone_awareness && ToneDetector::is_ready(&self.data_dir)).then(|| {
            let (me, s) = (self.clone(), samples.clone());
            std::thread::spawn(move || me.tone.detect(&me.data_dir, &s))
        });
        let raw = self.stt.transcribe(&self.data_dir, &samples)?;
        let tone = tone_job.and_then(|j| j.join().ok().flatten());
        if let Some(t) = tone {
            let mut recent = self.recent_tones.lock();
            recent.push_back((t, Instant::now()));
            while recent.len() > 6 {
                recent.pop_front();
            }
        }
        if raw.trim().is_empty() {
            anyhow::bail!("Tally didn't catch that. Try again a little closer to the mic?");
        }

        if !ai_mode {
            let output = match settings.dictation_style {
                DictationStyle::Verbatim => raw,
                DictationStyle::Clean => text::clean(&raw),
                DictationStyle::Polished => {
                    let cleaned = text::clean(&raw);
                    tauri::async_runtime::block_on(ai::polish(&settings.ai, &settings.profile, &cleaned)).unwrap_or(cleaned)
                }
            };
            let delivered = insert::deliver(&self.app, &output, !settings.restore_clipboard)?;
            if settings.voice.history {
                self.history.add(history::Kind::Dictate, "", &output);
            }
            let words = output.split_whitespace().count();
            return Ok(Outcome::Done(
                match delivered {
                    Delivered::Pasted => format!("{words} word{}", if words == 1 { "" } else { "s" }),
                    Delivered::Copied(why) => copied_hint(why),
                },
                tone,
            ));
        }

        let instruction = text::clean(&raw);
        // The highlight is still in place: read it now, and pasting replaces it.
        let context = insert::selected_text(&self.app);
        let words = context.as_deref().map(|c| c.split_whitespace().count()).unwrap_or(0);
        hud::set(&self.app, HudState::Thinking { instruction: instruction.clone(), context_words: words, tone });
        let recent = settings.voice.memory.then(|| self.memory.recall()).flatten();
        let reply = tauri::async_runtime::block_on(ai::ask(&settings.ai, &settings.profile, &instruction, context.as_deref(), tone, recent.as_deref()))?;
        if settings.voice.memory {
            self.memory.remember(&instruction, &reply);
        }
        if settings.voice.history {
            self.history.add(history::Kind::Ask, &instruction, &reply);
        }
        // AI results are typed in and stay on the clipboard, so they're never lost.
        Ok(Outcome::Done(
            match insert::deliver(&self.app, &reply, true)? {
                Delivered::Pasted => "Done · also on your clipboard".into(),
                Delivered::Copied(why) => copied_hint(why),
            },
            tone,
        ))
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
    Done(String, Option<Tone>),
    Nothing,
}

fn copied_hint(why: WhyCopied) -> String {
    let key = if cfg!(target_os = "macos") { "⌘V" } else { "Ctrl+V" };
    match why {
        WhyCopied::NoAccessibility => format!("Copied · press {key} (turn on Accessibility to paste for you)"),
        WhyCopied::NoTextField => format!("Copied · click where you want it and press {key}"),
    }
}
