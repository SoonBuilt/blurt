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

/// Qwen3-4B-Instruct: the best small model at following "reply with the text only", and it
/// has no thinking mode at all, so it can never leak reasoning into someone's document.
/// The `-instruct` suffix matters: plain `qwen3:4b` is the thinking build.
pub const DEFAULT_OLLAMA_MODEL: &str = "qwen3:4b-instruct";
/// What Blurt used to ship. Anyone still on it gets moved along.
const OLD_OLLAMA_MODEL: &str = "llama3.2:3b";

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
            ollama_model: DEFAULT_OLLAMA_MODEL.into(),
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
    /// Listen to how you sound and let Ask AI adapt to it (needs the tone pack).
    pub tone_awareness: bool,
    /// Double-tap the talk key to talk without holding it.
    pub hands_free: bool,
    /// Ask AI remembers the last few exchanges for a few minutes (in memory only).
    pub memory: bool,
    /// Keep the last 10 results, on disk, so nothing is lost if a paste goes nowhere.
    #[serde(default = "yes")]
    pub history: bool,
}

fn yes() -> bool {
    true
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self { tone_awareness: true, hands_free: true, memory: true, history: true }
    }
}

/// "How do you write?" answers, turned into guidance for every AI request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WritingProfile {
    pub name: String,
    /// What they do, e.g. "product designer at a small agency".
    pub role: String,
    /// "casual" | "friendly" | "professional" | "formal" (empty = no preference)
    pub tone: String,
    /// "brief" | "balanced" | "detailed"
    pub length: String,
    /// "us" | "uk"
    pub spelling: String,
    /// "never" | "sometimes" | "often"
    pub emoji: String,
    /// How they sign off emails, e.g. "Cheers, Dan".
    pub sign_off: String,
    /// Anything else Tally should know.
    pub extra: String,
}

impl WritingProfile {
    /// Plain-English guidance for the AI; empty when nothing's been filled in.
    pub fn guidance(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        let who = match (self.name.trim(), self.role.trim()) {
            ("", "") => String::new(),
            (n, "") => format!("The user's name is {n}."),
            ("", r) => format!("The user is a {r}."),
            (n, r) => format!("The user is {n}, a {r}."),
        };
        if !who.is_empty() {
            lines.push(who);
        }
        match self.tone.as_str() {
            "casual" => lines.push("Write casually, like texting a friend.".into()),
            "friendly" => lines.push("Write in a warm, friendly way.".into()),
            "professional" => lines.push("Write in a clear, professional way.".into()),
            "formal" => lines.push("Write formally and politely.".into()),
            _ => {}
        }
        match self.length.as_str() {
            "brief" => lines.push("Keep things short and to the point.".into()),
            "detailed" => lines.push("It's fine to be thorough and detailed.".into()),
            _ => {}
        }
        match self.spelling.as_str() {
            "uk" => lines.push("Use British spelling.".into()),
            "us" => lines.push("Use American spelling.".into()),
            _ => {}
        }
        match self.emoji.as_str() {
            "never" => lines.push("Never use emojis.".into()),
            "sometimes" => lines.push("An emoji now and then is fine in casual messages.".into()),
            "often" => lines.push("Feel free to use emojis.".into()),
            _ => {}
        }
        if !self.sign_off.trim().is_empty() {
            lines.push(format!("When writing an email or letter, sign off as: {}", self.sign_off.trim()));
        }
        if !self.extra.trim().is_empty() {
            lines.push(format!("More about the user: {}", self.extra.trim()));
        }
        lines.join(" ")
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
    pub profile: WritingProfile,
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
            profile: WritingProfile::default(),
        }
    }
}

fn path(dir: &PathBuf) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &PathBuf) -> Settings {
    let mut s: Settings = std::fs::read_to_string(path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Anyone who never changed the old default moves to the better model.
    if s.ai.ollama_model == OLD_OLLAMA_MODEL {
        s.ai.ollama_model = DEFAULT_OLLAMA_MODEL.into();
    }
    // The old single "writing style" box moves into the questionnaire's free-text answer.
    if !s.ai.style.trim().is_empty() && s.profile.extra.trim().is_empty() {
        s.profile.extra = std::mem::take(&mut s.ai.style);
    }
    s
}

pub fn save(dir: &PathBuf, settings: &Settings) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(path(dir), serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
