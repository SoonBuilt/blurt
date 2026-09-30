//! User settings, stored as JSON in the app's config directory.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DictationStyle {
    /// Exactly what the speech model heard.
    Verbatim,
    /// Filler words removed, spoken commands like "new line" applied.
    Clean,
    /// Clean, then tidied into good writing by the AI engine.
    Polished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    /// Apple Intelligence's on-device model (macOS 26+ with Apple Intelligence on).
    Apple,
    /// A local model served by Ollama.
    Ollama,
    /// Anthropic's API with the user's own key.
    Anthropic,
    /// Any OpenAI-compatible endpoint with the user's own key.
    Openai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub provider: AiProvider,
    pub ollama_url: String,
    pub ollama_model: String,
    pub anthropic_model: String,
    pub openai_url: String,
    pub openai_model: String,
    /// Extra style guidance added to every AI request, e.g. "British spelling, no emojis".
    pub style: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: if cfg!(target_os = "macos") {
                AiProvider::Apple
            } else {
                AiProvider::Ollama
            },
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "llama3.2:3b".into(),
            anthropic_model: "claude-opus-5".into(),
            openai_url: "https://api.openai.com/v1".into(),
            openai_model: "gpt-5-mini".into(),
            style: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    /// Tally answers questions out loud instead of typing the answer.
    pub read_answers: bool,
    /// Tally also reads out text it wrote for you after inserting it.
    pub read_everything: bool,
    /// Listen to how you sound and let Ask AI adapt to it.
    pub tone_awareness: bool,
    /// Double-tap the talk key to talk without holding it.
    pub hands_free: bool,
    /// Use the premium, more expressive voice (Chatterbox Turbo) when it's installed.
    pub premium_voice: bool,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self { read_answers: true, read_everything: false, tone_awareness: true, hands_free: true, premium_voice: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Hold this to talk: "OptRight" (macOS default) or "CtrlRight" (Windows default).
    pub talk_key: String,
    /// Add this while holding the talk key to ask AI instead of dictating.
    pub ai_modifier: String,
    pub dictation_style: DictationStyle,
    pub restore_clipboard: bool,
    pub sounds: bool,
    pub onboarded: bool,
    pub ai: AiSettings,
    pub voice: VoiceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            talk_key: if cfg!(target_os = "macos") {
                "OptRight".into()
            } else {
                "CtrlRight".into()
            },
            ai_modifier: "Shift".into(),
            dictation_style: DictationStyle::Clean,
            restore_clipboard: true,
            sounds: true,
            onboarded: false,
            ai: AiSettings::default(),
            voice: VoiceSettings::default(),
        }
    }
}

fn path(dir: &PathBuf) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &PathBuf) -> Settings {
    std::fs::read_to_string(path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &PathBuf, settings: &Settings) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(path(dir), serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
