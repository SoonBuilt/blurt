//! Hold-to-talk hotkey state machine.
//!
//! One key, two modes:
//! - hold the talk key (Right ⌥ on macOS, Right Ctrl on Windows) → dictate
//! - add the AI modifier (⇧) at any point while holding → ask AI
//!
//! A short grace period filters out normal shortcuts: if another key or a different
//! modifier is pressed before recording starts, the press is ignored.

use handy_keys::{Key, KeyEvent, KeyboardListener, Modifiers};
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// How long the talk key must be held before we start listening.
const ARM_DELAY: Duration = Duration::from_millis(160);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dictate,
    Ai,
}

#[derive(Debug, Clone, Copy)]
pub enum KeyAction {
    Start(Mode),
    SwitchToAi,
    Stop,
    Cancel,
}

enum Phase {
    Idle,
    Armed {
        since: Instant,
        ai: bool,
    },
    Recording {
        ai: bool,
    },
    /// The press turned out to be a shortcut; wait for the talk key to come up.
    Ignored,
}

pub struct KeyConfig {
    pub talk: Modifiers,
    pub ai: Modifiers,
}

impl KeyConfig {
    pub fn parse(talk: &str, ai: &str) -> Self {
        let default_talk = if cfg!(target_os = "macos") {
            Modifiers::OPT_RIGHT
        } else {
            Modifiers::CTRL_RIGHT
        };
        Self {
            talk: Modifiers::from_str(talk).unwrap_or(default_talk),
            ai: Modifiers::from_str(ai).unwrap_or(Modifiers::SHIFT),
        }
    }
}

/// Blocks the calling thread, turning raw keyboard events into [`KeyAction`]s.
/// `config` is re-read on every event so hotkey changes apply without a restart.
pub fn run(tx: Sender<KeyAction>, config: impl Fn() -> KeyConfig) -> handy_keys::Result<()> {
    let listener = KeyboardListener::new()?;
    let mut phase = Phase::Idle;

    loop {
        let event = listener.recv_timeout(Duration::from_millis(15)).ok();
        let cfg = config();

        // Promote an armed press into a recording once the grace period passes.
        if let Phase::Armed { since, ai } = phase {
            if since.elapsed() >= ARM_DELAY {
                phase = Phase::Recording { ai };
                let _ = tx.send(KeyAction::Start(if ai { Mode::Ai } else { Mode::Dictate }));
            }
        }

        let Some(ev) = event else { continue };
        phase = step(phase, &ev, &cfg, &tx);
    }
}

fn is(changed: Option<Modifiers>, wanted: Modifiers) -> bool {
    changed.is_some_and(|c| !c.is_empty() && wanted.intersects(c))
}

fn step(phase: Phase, ev: &KeyEvent, cfg: &KeyConfig, tx: &Sender<KeyAction>) -> Phase {
    let talk_changed = is(ev.changed_modifier, cfg.talk);
    let ai_changed = is(ev.changed_modifier, cfg.ai);
    // Modifiers held other than the talk key and the AI modifier.
    let others = ev.modifiers.difference(cfg.talk | cfg.ai);

    match phase {
        Phase::Idle => {
            if talk_changed && ev.is_key_down && others.is_empty() {
                Phase::Armed {
                    since: Instant::now(),
                    ai: ev.modifiers.intersects(cfg.ai),
                }
            } else {
                Phase::Idle
            }
        }
        Phase::Armed { since, ai } => {
            if talk_changed && !ev.is_key_down {
                Phase::Idle // a quick tap, not a hold
            } else if ev.key.is_some() && ev.is_key_down {
                Phase::Ignored // talk key + a letter: it's a shortcut
            } else if ai_changed && ev.is_key_down {
                Phase::Armed { since, ai: true }
            } else if !others.is_empty() {
                Phase::Ignored
            } else {
                Phase::Armed { since, ai }
            }
        }
        Phase::Recording { ai } => {
            if talk_changed && !ev.is_key_down {
                let _ = tx.send(KeyAction::Stop);
                Phase::Idle
            } else if ev.key == Some(Key::Escape) && ev.is_key_down {
                let _ = tx.send(KeyAction::Cancel);
                Phase::Ignored
            } else if !ai && ai_changed && ev.is_key_down {
                let _ = tx.send(KeyAction::SwitchToAi);
                Phase::Recording { ai: true }
            } else {
                Phase::Recording { ai }
            }
        }
        Phase::Ignored => {
            if talk_changed && !ev.is_key_down {
                Phase::Idle
            } else {
                Phase::Ignored
            }
        }
    }
}
