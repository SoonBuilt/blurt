import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

/* In a plain browser (design review with `pnpm dev`), answer with sample data instead of Rust. */
const mock: Record<string, unknown> = {
  get_status: {
    platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
    settings: {
      talk_key: "OptRight", ai_modifier: "Shift", dictation_style: "clean", restore_clipboard: true, sounds: true,
      onboarded: new URLSearchParams(location.search).has("settings"),
      ai: { provider: "apple", ollama_url: "http://localhost:11434", ollama_model: "qwen3:4b-instruct", anthropic_model: "claude-opus-5",
        openai_url: "https://api.openai.com/v1", openai_model: "gpt-5-mini", style: "" },
      voice: { tone_awareness: true, hands_free: true, memory: true },
      profile: { name: "", role: "", tone: "", length: "", spelling: "", emoji: "", sign_off: "", extra: "" },
    },
    modelDownloaded: false, toneReady: false, recentTones: [["calm", 40], ["frustrated", 300], ["upbeat", 900]], accessibility: false, microphone: "unknown", appleAi: "", hasAnthropicKey: false, hasOpenaiKey: false,
  },
  test_ai: "Hey there, lovely to meet you!",
};
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) return tauriInvoke<T>(cmd, args);
  return Promise.resolve(mock[cmd] as T);
}

export type DictationStyle = "verbatim" | "clean" | "polished";
export type AiProvider = "apple" | "ollama" | "anthropic" | "openai";

export type AiSettings = {
  provider: AiProvider;
  ollama_url: string;
  ollama_model: string;
  anthropic_model: string;
  openai_url: string;
  openai_model: string;
  style: string;
};

export type VoiceSettings = {
  tone_awareness: boolean;
  hands_free: boolean;
  memory: boolean;
};

export type Pack = "speech" | "tone";

export type WritingProfile = {
  name: string;
  role: string;
  tone: "" | "casual" | "friendly" | "professional" | "formal";
  length: "" | "brief" | "balanced" | "detailed";
  spelling: "" | "us" | "uk";
  emoji: "" | "never" | "sometimes" | "often";
  sign_off: string;
  extra: string;
};

export type Settings = {
  talk_key: string;
  ai_modifier: string;
  dictation_style: DictationStyle;
  restore_clipboard: boolean;
  sounds: boolean;
  onboarded: boolean;
  ai: AiSettings;
  voice: VoiceSettings;
  profile: WritingProfile;
};

export type Status = {
  platform: "macos" | "windows" | "linux";
  settings: Settings;
  modelDownloaded: boolean;
  toneReady: boolean;
  /** Newest first: [tone, seconds ago]. */
  recentTones: [import("./tone").Tone, number][];
  accessibility: boolean;
  microphone: "allowed" | "denied" | "restricted" | "unknown";
  appleAi: string;
  hasAnthropicKey: boolean;
  hasOpenaiKey: boolean;
};

export const api = {
  status: () => invoke<Status>("get_status"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  downloadPack: (pack: Pack) => invoke<void>("download_pack", { pack }),
  setApiKey: (provider: AiProvider, key: string) => invoke<void>("set_api_key", { provider, key }),
  testAi: () => invoke<string>("test_ai"),
  requestMicrophone: () => invoke<boolean>("request_microphone"),
  requestAccessibility: () => invoke<void>("request_accessibility"),
  openPrivacySettings: (pane: "microphone" | "accessibility" | "ai") => invoke<void>("open_privacy_settings", { pane }),
  finishOnboarding: () => invoke<void>("finish_onboarding"),
};

/** Human labels for the talk key and AI modifier on this platform. */
export function keyLabels(platform: Status["platform"]) {
  return platform === "macos"
    ? { talk: "Right ⌥", talkLong: "Right ⌥ Option", ai: "⇧", aiLong: "⇧ Shift" }
    : { talk: "Right Ctrl", talkLong: "Right Ctrl", ai: "Shift", aiLong: "Shift" };
}
