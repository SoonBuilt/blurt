import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

/* In a plain browser (design review with `pnpm dev`), answer with sample data instead of Rust. */
const mock: Record<string, unknown> = {
  get_status: {
    platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
    settings: {
      talk_key: "OptRight", ai_modifier: "Shift", dictation_style: "clean", restore_clipboard: true, sounds: true,
      onboarded: new URLSearchParams(location.search).has("settings"),
      ai: { provider: "apple", ollama_url: "http://localhost:11434", ollama_model: "llama3.2:3b", anthropic_model: "claude-opus-5",
        openai_url: "https://api.openai.com/v1", openai_model: "gpt-5-mini", style: "" },
    },
    modelDownloaded: false, accessibility: false, microphone: "unknown", appleAi: "", hasAnthropicKey: false, hasOpenaiKey: false,
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

export type Settings = {
  talk_key: string;
  ai_modifier: string;
  dictation_style: DictationStyle;
  restore_clipboard: boolean;
  sounds: boolean;
  onboarded: boolean;
  ai: AiSettings;
};

export type Status = {
  platform: "macos" | "windows" | "linux";
  settings: Settings;
  modelDownloaded: boolean;
  accessibility: boolean;
  microphone: "allowed" | "denied" | "restricted" | "unknown";
  appleAi: string;
  hasAnthropicKey: boolean;
  hasOpenaiKey: boolean;
};

export const api = {
  status: () => invoke<Status>("get_status"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  downloadModel: () => invoke<void>("download_model"),
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
