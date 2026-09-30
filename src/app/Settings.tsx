import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { keyLabels, type DictationStyle, type Status } from "../lib/api";
import { Tally } from "../tally/Tally";
import { AiEngine, Permissions, PremiumVoice, ToneAwareness, Toggle, TallyVoice, useSettings, VoiceModel } from "./parts";

const PAGES = [
  { id: "general", label: "General", icon: "⚙️" },
  { id: "voice", label: "Voice", icon: "🎙" },
  { id: "tally", label: "Talk with Tally", icon: "🗣" },
  { id: "ai", label: "AI engine", icon: "✨" },
  { id: "about", label: "About", icon: "👋" },
] as const;
type Page = (typeof PAGES)[number]["id"];

const MAC_TALK_KEYS = [
  { v: "OptRight", l: "Right ⌥ Option" },
  { v: "OptLeft", l: "Left ⌥ Option" },
  { v: "CmdRight", l: "Right ⌘ Command" },
  { v: "CtrlRight", l: "Right ⌃ Control" },
];
const WIN_TALK_KEYS = [
  { v: "CtrlRight", l: "Right Ctrl" },
  { v: "CtrlLeft", l: "Left Ctrl" },
  { v: "OptRight", l: "Right Alt" },
];

export default function SettingsView({ status, refresh }: { status: Status; refresh: () => void }) {
  const [page, setPage] = useState<Page>("general");
  const save = useSettings(status, refresh);
  const s = status.settings;
  const mac = status.platform === "macos";
  const keys = keyLabels(status.platform);
  const needsAttention = !status.modelDownloaded || (mac && (!status.accessibility || status.microphone !== "allowed"));

  return (
    <div className="set">
      <aside className="set-side">
        <div className="brand">
          <Tally size={26} mood={needsAttention ? "needs" : "hello"} tape={false} shadow={false} />
          Blurt
        </div>
        {PAGES.map((p) => (
          <button key={p.id} className={page === p.id ? "on" : ""} onClick={() => setPage(p.id)}>
            <span>{p.icon}</span>
            {p.label}
            {p.id === "voice" && needsAttention && <i className="dot" />}
          </button>
        ))}
      </aside>

      <main className="set-main">
        {page === "general" && (
          <>
            <div className="buddy small">
              <Tally size={48} mood="hello" tilt={-5} />
              <div className="bubble">
                Hold <kbd>{keys.talk}</kbd> to dictate. Add <kbd>{keys.ai}</kbd> to ask AI. Highlight text first and I'll use it as context.
              </div>
            </div>
            <h2>General</h2>
            <div className="group">
              <label className="row">
                <span className="t">
                  <b>Talk key</b>
                  <span>Hold it to talk, let go to insert.</span>
                </span>
                <select value={s.talk_key} onChange={(e) => save((x) => ({ ...x, talk_key: e.target.value }))}>
                  {(mac ? MAC_TALK_KEYS : WIN_TALK_KEYS).map((k) => (
                    <option key={k.v} value={k.v}>
                      {k.l}
                    </option>
                  ))}
                </select>
              </label>
              <div className="row">
                <span className="t">
                  <b>AI key</b>
                  <span>Press it while holding the talk key to ask AI instead.</span>
                </span>
                <kbd>{keys.aiLong}</kbd>
              </div>
              <div className="row">
                <span className="t">
                  <b>Cancel</b>
                  <span>Changed your mind mid-sentence?</span>
                </span>
                <kbd>Esc</kbd>
              </div>
              <div className="row">
                <span className="t">
                  <b>Restore my clipboard</b>
                  <span>Blurt pastes through the clipboard, then puts your copied text back.</span>
                </span>
                <Toggle label="Restore clipboard" on={s.restore_clipboard} onChange={(v) => save((x) => ({ ...x, restore_clipboard: v }))} />
              </div>
            </div>
          </>
        )}

        {page === "voice" && (
          <>
            <h2>Voice</h2>
            <div className="group">
              <div className="row">
                <span className="t">
                  <b>Dictation style</b>
                  <span>
                    {s.dictation_style === "verbatim"
                      ? "Exactly what you said."
                      : s.dictation_style === "clean"
                        ? "Without ums and uhs. Say “new line” for a line break."
                        : "Tidied into good writing by your AI engine. A little slower."}
                  </span>
                </span>
                <div className="seg">
                  {(["verbatim", "clean", "polished"] as DictationStyle[]).map((d) => (
                    <button key={d} className={s.dictation_style === d ? "on" : ""} onClick={() => save((x) => ({ ...x, dictation_style: d }))}>
                      {d[0].toUpperCase() + d.slice(1)}
                    </button>
                  ))}
                </div>
              </div>
            </div>
            <VoiceModel status={status} refresh={refresh} />
            {mac && (
              <>
                <h3>Permissions</h3>
                <Permissions status={status} />
              </>
            )}
          </>
        )}

        {page === "tally" && (
          <>
            <div className="buddy small">
              <Tally size={48} mood="listening" paper="lilac" tilt={-5} />
              <div className="bubble">
                Ask me something with <kbd>{keys.talk}</kbd> + <kbd>{keys.ai}</kbd> and I'll answer out loud. Double-tap <kbd>{keys.talk}</kbd> to talk without holding it.
              </div>
            </div>
            <h2>Talk with Tally</h2>
            <div className="group">
              <div className="row">
                <span className="t">
                  <b>Answer questions out loud</b>
                  <span>When you ask Tally something, it talks back instead of typing the answer. Press any key or Esc to stop it.</span>
                </span>
                <Toggle label="Answer out loud" on={s.voice.read_answers} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, read_answers: v } }))} />
              </div>
              <div className="row">
                <span className="t">
                  <b>Also read out what it writes</b>
                  <span>After inserting an email or rewrite, Tally reads it to you.</span>
                </span>
                <Toggle label="Read out writing" on={s.voice.read_everything} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, read_everything: v } }))} />
              </div>
              <div className="row">
                <span className="t">
                  <b>Hands-free</b>
                  <span>
                    Double-tap <kbd>{keys.talk}</kbd> and just talk. Tally notices when you've finished, even if you pause to think. Tap once to stop early.
                  </span>
                </span>
                <Toggle label="Hands-free" on={s.voice.hands_free} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, hands_free: v } }))} />
              </div>
              <div className="row">
                <span className="t">
                  <b>Notice my tone</b>
                  <span>If you sound stressed or annoyed, Tally stays calm and kind. It never comments on it unless you ask.</span>
                </span>
                <Toggle label="Notice my tone" on={s.voice.tone_awareness} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, tone_awareness: v } }))} />
              </div>
            </div>
            <TallyVoice status={status} refresh={refresh} />
            <PremiumVoice status={status} refresh={refresh} />
            <ToneAwareness status={status} refresh={refresh} />
          </>
        )}

        {page === "ai" && (
          <>
            <h2>AI engine</h2>
            <p className="lead">Who answers when you hold {keys.talk} + {keys.ai}.</p>
            <AiEngine status={status} refresh={refresh} />
            <h3>Your writing style</h3>
            <textarea
              className="style"
              placeholder="e.g. Casual and concise. British spelling. No emojis."
              defaultValue={s.ai.style}
              onBlur={(e) => save((x) => ({ ...x, ai: { ...x.ai, style: e.target.value } }))}
            />
          </>
        )}

        {page === "about" && (
          <div className="about">
            <Tally size={96} mood="ready" tilt={-6} />
            <h2>Blurt</h2>
            <p className="lead">Hold a key and talk. Made by soonbuilt, with Tally.</p>
            <div className="links">
              <button className="btn" onClick={() => openUrl("https://soonbuilt.com")}>
                soonbuilt.com
              </button>
            </div>
            <p className="credits">
              Speech recognition: NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0). Tally's voice: Kyutai Pocket TTS (CC BY 4.0), with a CC0 voice from Kyutai's
              Unmute project. Tone: emotion2vec+ by Ma et al. (FunASR model licence). Turn detection: Pipecat Smart Turn v3.2 (BSD-2). Runs on ONNX
              Runtime via sherpa-onnx (Apache-2.0). Hotkeys and model plumbing build on Handy (MIT). Version 0.1.0.
            </p>
          </div>
        )}
      </main>
    </div>
  );
}
