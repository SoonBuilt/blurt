//! The coordinator: key presses in, text out.
//!
//! hold key → record → release → transcribe → (dictate: clean up | ask AI: run the prompt
//! with any highlighted text as context) → paste where the cursor is.

use crate::hud::{self, HudState};
use crate::keys::{KeyAction, Mode};
use crate::settings::{DictationStyle, Settings};
use crate::tray::{self, Mood};
use crate::{ai, audio::Recorder, insert, stt::Transcriber, text};
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

/// Recordings shorter than this are treated as accidental presses.
const MIN_SAMPLES: usize = crate::audio::TARGET_RATE as usize * 3 / 10;

pub struct Engine {
    pub app: AppHandle,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub stt: Transcriber,
    recorder: Recorder,
    mode: Mutex<Mode>,
    recording: AtomicBool,
    busy: AtomicBool,
}

impl Engine {
    pub fn new(app: AppHandle, data_dir: PathBuf, config_dir: PathBuf) -> Arc<Self> {
        let settings = crate::settings::load(&config_dir);
        let level_app = app.clone();
        let recorder = Recorder::spawn(move |l| hud::level(&level_app, l));
        Arc::new(Self {
            app,
            data_dir,
            config_dir,
            settings: RwLock::new(settings),
            stt: Transcriber::new(),
            recorder,
            mode: Mutex::new(Mode::Dictate),
            recording: AtomicBool::new(false),
            busy: AtomicBool::new(false),
        })
    }

    pub fn handle(self: &Arc<Self>, action: KeyAction) {
        match action {
            KeyAction::Start(mode) => self.start(mode),
            KeyAction::SwitchToAi => {
                if self.recording.load(Ordering::SeqCst) {
                    *self.mode.lock() = Mode::Ai;
                    hud::set(
                        &self.app,
                        HudState::Listening {
                            ai: true,
                            context_words: 0,
                        },
                    );
                }
            }
            KeyAction::Cancel => {
                if self.recording.swap(false, Ordering::SeqCst) {
                    self.recorder.cancel();
                    hud::set(&self.app, HudState::Hidden);
                    tray::set_mood(&self.app, Mood::Hello);
                }
            }
            KeyAction::Stop => {
                if self.recording.swap(false, Ordering::SeqCst) {
                    let me = self.clone();
                    std::thread::spawn(move || me.finish());
                }
            }
        }
    }

    fn start(self: &Arc<Self>, mode: Mode) {
        if self.busy.load(Ordering::SeqCst) {
            return; // still writing the last one
        }
        if !crate::stt::is_downloaded(&self.data_dir) {
            self.flash_error("Finish setting up Blurt first: it needs its voice model.");
            tray::show_main(&self.app);
            return;
        }
        *self.mode.lock() = mode;
        self.recording.store(true, Ordering::SeqCst);
        self.recorder.start();
        tray::set_mood(&self.app, Mood::Listening);
        hud::set(
            &self.app,
            HudState::Listening {
                ai: mode == Mode::Ai,
                context_words: 0,
            },
        );
    }

    fn finish(self: &Arc<Self>) {
        self.busy.store(true, Ordering::SeqCst);
        let result = self.process();
        self.busy.store(false, Ordering::SeqCst);
        match result {
            Ok(Some(message)) => {
                tray::set_mood(&self.app, Mood::Ready);
                hud::set(&self.app, HudState::Done { message });
                std::thread::sleep(Duration::from_millis(1100));
                hud::set(&self.app, HudState::Hidden);
                tray::set_mood(&self.app, Mood::Hello);
            }
            Ok(None) => {
                hud::set(&self.app, HudState::Hidden);
                tray::set_mood(&self.app, Mood::Hello);
            }
            Err(e) => {
                log::error!("{e:#}");
                self.flash_error(&e.to_string());
            }
        }
    }

    /// Returns the "done" message, or None when there was nothing to do.
    fn process(self: &Arc<Self>) -> anyhow::Result<Option<String>> {
        let mode = *self.mode.lock();
        let ai_mode = mode == Mode::Ai;
        let samples = self.recorder.stop()?;
        if samples.len() < MIN_SAMPLES {
            return Ok(None);
        }

        tray::set_mood(&self.app, Mood::Working);
        hud::set(&self.app, HudState::Transcribing { ai: ai_mode });
        let raw = self.stt.transcribe(&self.data_dir, &samples)?;
        if raw.trim().is_empty() {
            anyhow::bail!("Tally didn't catch that. Try again a little closer to the mic?");
        }
        let settings = self.settings.read().clone();

        let output = if ai_mode {
            let instruction = text::clean(&raw);
            // The highlight is still in place: read it now, and pasting replaces it.
            let context = insert::selected_text(&self.app);
            let words = context
                .as_deref()
                .map(|c| c.split_whitespace().count())
                .unwrap_or(0);
            hud::set(
                &self.app,
                HudState::Thinking {
                    instruction: instruction.clone(),
                    context_words: words,
                },
            );
            tauri::async_runtime::block_on(ai::ask(&settings.ai, &instruction, context.as_deref()))?
        } else {
            match settings.dictation_style {
                DictationStyle::Verbatim => raw,
                DictationStyle::Clean => text::clean(&raw),
                DictationStyle::Polished => {
                    let cleaned = text::clean(&raw);
                    tauri::async_runtime::block_on(ai::polish(&settings.ai, &cleaned))
                        .unwrap_or(cleaned)
                }
            }
        };

        insert::paste(&self.app, &output, settings.restore_clipboard)?;
        let words = output.split_whitespace().count();
        Ok(Some(if ai_mode {
            "Done".to_string()
        } else {
            format!("{words} word{}", if words == 1 { "" } else { "s" })
        }))
    }

    fn flash_error(&self, message: &str) {
        tray::set_mood(&self.app, Mood::Needs);
        hud::set(
            &self.app,
            HudState::Error {
                message: message.to_string(),
            },
        );
        let app = self.app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(3200));
            hud::set(&app, HudState::Hidden);
            tray::set_mood(&app, Mood::Hello);
        });
    }
}
