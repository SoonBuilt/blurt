import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { api, keyLabels, TALK_KEYS, type DictationStyle, type HistoryEntry, type Status } from "../lib/api";
import { Tally } from "../tally/Tally";
import { FamilyCards } from "./Family";
import { AiEngine, Permissions, ToneAwareness, ToneReading, Toggle, useSettings, VoiceModel, WritingStyle } from "./parts";

const PAGES = [
  { id: "general", label: "General", icon: "⚙️" },
  { id: "voice", label: "Voice", icon: "🎙" },
  { id: "smarts", label: "Smarts", icon: "🧠" },
  { id: "style", label: "Your style", icon: "✍️" },
  { id: "history", label: "History", icon: "🕘" },
  { id: "ai", label: "AI engine", icon: "✨" },
  { id: "about", label: "About", icon: "👋" },
] as const;
export type Page = (typeof PAGES)[number]["id"];


export default function SettingsView({
  status,
  refresh,
  page,
  setPage,
  onPracticed,
  onTour,
}: {
  status: Status;
  refresh: () => void;
  page: Page;
  setPage: (p: Page) => void;
  onPracticed: () => void;
  onTour: () => void;
}) {
  const save = useSettings(status, refresh);
  const s = status.settings;
  const mac = status.platform === "macos";
  const keys = keyLabels(status.platform, s.talk_key);
  const needsAttention = !status.modelDownloaded || (mac && (!status.accessibility || status.microphone !== "allowed"));

  return (
    <div className="set">
      <aside className="set-side">
        <div className="brand">
          <Tally size={26} mood={needsAttention ? "needs" : "hello"} tape={false} shadow={false} />
          Blurt
        </div>
        {PAGES.map((p) => (
          <button key={p.id} data-tour={`nav-${p.id}`} className={page === p.id ? "on" : ""} onClick={() => setPage(p.id)}>
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
            <GettingStarted status={status} setPage={setPage} onTour={onTour} />
            <div className="card practice" data-tour="practice">
              <b>Try it here</b>
              <textarea
                className="tryit"
                placeholder={`Click here, hold ${keys.talk} and say: “Hey, this is my first message with Blurt.”`}
                onInput={(e) => (e.currentTarget.value.trim() ? onPracticed() : undefined)}
              />
            </div>
            <h2>General</h2>
            <div className="group">
              <label className="row">
                <span className="t">
                  <b>Talk key</b>
                  <span>Hold it to talk, let go to insert. No right-hand key on your keyboard? Any of these work.</span>
                </span>
                <select value={s.talk_key} onChange={(e) => save((x) => ({ ...x, talk_key: e.target.value }))}>
                  {(mac ? TALK_KEYS.macos : TALK_KEYS.windows).map((k) => (
                    <option key={k.v} value={k.v}>
                      {k.long}
                    </option>
                  ))}
                </select>
              </label>
              {mac && s.talk_key === "Fn" && (
                <p className="note">
                  macOS gives <kbd>🌐 Fn</kbd> a job of its own — usually the emoji picker. Set <b>System Settings › Keyboard › “Press 🌐 to”</b> to{" "}
                  <b>Do Nothing</b> and it's all yours.
                </p>
              )}
              {!mac && s.talk_key === "OptRight" && (
                <p className="note">On some European layouts Right Alt is AltGr, which types accented characters. If that's yours, pick another key.</p>
              )}
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
            <div data-tour="model">
              <VoiceModel status={status} refresh={refresh} />
            </div>
            {mac && (
              <>
                <h3>Permissions</h3>
                <div data-tour="perms">
                  <Permissions status={status} />
                </div>
              </>
            )}
          </>
        )}

        {page === "smarts" && (
          <>
            <div className="buddy small">
              <Tally size={48} mood="pointing" paper="lilac" tilt={-5} />
              <div className="bubble">
                A few small things that make me quicker to work with. Everything stays on your {mac ? "Mac" : "PC"}.
              </div>
            </div>
            <h2>Smarts</h2>
            <div className="group">
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
                  <b>Short-term memory</b>
                  <span>
                    Ask AI remembers your last few requests for 10 minutes, so “make it shorter” or “now in Spanish” just works. Never saved to disk.
                  </span>
                </span>
                <Toggle label="Short-term memory" on={s.voice.memory} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, memory: v } }))} />
              </div>
              <div className="row">
                <span className="t">
                  <b>Notice my tone</b>
                  <span>If you sound stressed or annoyed, Ask AI keeps its wording calm and kind. Needs the small tone download below.</span>
                </span>
                <Toggle label="Notice my tone" on={s.voice.tone_awareness} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, tone_awareness: v } }))} />
              </div>
            </div>
            <h3>How you've sounded</h3>
            <ToneReading status={status} />
            <ToneAwareness status={status} refresh={refresh} />
          </>
        )}

        {page === "style" && (
          <>
            <div className="buddy small">
              <Tally size={48} mood="pointing" paper="butter" tilt={-5} />
              <div className="bubble">Tell me a little about how you write, and everything I write for you will sound like you.</div>
            </div>
            <h2>Your style</h2>
            <p className="lead">All optional. Skip anything you don't care about; Tally uses what you fill in for every Ask AI request and Polished dictation.</p>
            <WritingStyle status={status} refresh={refresh} />
          </>
        )}

        {page === "ai" && (
          <>
            <h2>AI engine</h2>
            <p className="lead">Who answers when you hold {keys.talk} + {keys.ai}.</p>
            <AiEngine status={status} refresh={refresh} />
          </>
        )}

        {page === "history" && (
          <>
            <div className="buddy small">
              <Tally size={48} mood="ready" paper="mint" tilt={-5} />
              <div className="bubble">The last 10 things I wrote for you. Handy if something went somewhere you didn't expect.</div>
            </div>
            <h2>History</h2>
            <div className="group">
              <div className="row">
                <span className="t">
                  <b>Keep my last 10</b>
                  <span>Saved on this {mac ? "Mac" : "PC"} so nothing is lost if a result lands somewhere odd. Never leaves your computer.</span>
                </span>
                <Toggle label="Keep history" on={s.voice.history} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, history: v } }))} />
              </div>
            </div>
            <HistoryList on={s.voice.history} />
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
            <div className="about-family">
              <h3>More from soonbuilt</h3>
              <FamilyCards />
            </div>
            <p className="credits">
              Speech recognition: NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0). Tone: emotion2vec+ by Ma et al. (FunASR model licence). Turn detection: Pipecat Smart Turn v3.2 (BSD-2). Runs on ONNX
              Runtime. Hotkeys and model plumbing build on Handy (MIT). Version <Version />.
            </p>
          </div>
        )}
      </main>
    </div>
  );
}

/** A small, dismissable checklist on the General page until the basics are done. */
function GettingStarted({ status, setPage, onTour }: { status: Status; setPage: (p: Page) => void; onTour: () => void }) {
  const [hidden, setHidden] = useState(() => {
    try {
      return localStorage.getItem("blurt.checklist.hidden") === "1";
    } catch {
      return false;
    }
  });
  const mac = status.platform === "macos";
  const p = status.settings.profile;
  const items: { label: string; done: boolean; page: Page }[] = [
    ...(mac ? [{ label: "Allow the microphone and Accessibility", done: status.accessibility && status.microphone === "allowed", page: "voice" as Page }] : []),
    { label: "Download the voice model", done: status.modelDownloaded, page: "voice" },
    { label: "Tell Tally how you write", done: !!(p.name || p.tone || p.length || p.extra || p.sign_off), page: "style" },
    { label: "Pick who does the AI thinking", done: status.settings.ai.provider !== "apple" || !status.appleAi, page: "ai" },
  ];
  const left = items.filter((i) => !i.done).length;
  if (hidden) return null;
  return (
    <div className="card checklist">
      <div className="row-flex">
        <Tally size={34} mood={left ? "pointing" : "ready"} paper={left ? "sky" : "mint"} tape={false} tilt={-5} />
        <span className="grow">
          <b>{left ? `Getting started · ${items.length - left} of ${items.length}` : "You're all set!"}</b>
          <span>{left ? "A few quick things and Blurt is fully yours." : "Hold the key anywhere and talk."}</span>
        </span>
        <button className="btn" onClick={onTour}>
          ▶ Tour with Tally
        </button>
        <button
          className="x"
          aria-label="Hide checklist"
          onClick={() => {
            setHidden(true);
            try {
              localStorage.setItem("blurt.checklist.hidden", "1");
            } catch {
              /* ignore */
            }
          }}
        >
          ×
        </button>
      </div>
      <ul>
        {items.map((it) => (
          <li key={it.label} className={it.done ? "done" : ""}>
            <button onClick={() => setPage(it.page)}>
              <span className="tick">{it.done ? "✓" : ""}</span>
              {it.label}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The real version from the bundle, so it can never drift from what was shipped. */
function Version() {
  const [v, setV] = useState("");
  useEffect(() => {
    if (!isTauri()) return;
    getVersion().then(setV, () => {});
  }, []);
  return <>{v || "—"}</>;
}

/** Says how long ago something happened, in the plainest words. */
function ago(at: number) {
  const mins = Math.round((Date.now() - at) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  const days = Math.round(hours / 24);
  return `${days} day${days === 1 ? "" : "s"} ago`;
}

/** The last 10 results, newest first, each one copyable. */
function HistoryList({ on }: { on: boolean }) {
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [copied, setCopied] = useState<number | null>(null);

  const load = () => api.historyList().then(setItems, () => setItems([]));
  useEffect(() => {
    if (on) load();
    else setItems([]);
  }, [on]);

  if (!on) return <p className="lead">History is off, so nothing is being kept.</p>;
  if (!items.length)
    return (
      <div className="card empty">
        <Tally size={40} mood="hello" tape={false} tilt={-5} />
        <span>Nothing yet. Hold your talk key and say something, and it'll show up here.</span>
      </div>
    );

  return (
    <>
      <div className="hist">
        {items.map((it) => (
          <div className="hist-item" key={it.at}>
            <div className="hist-head">
              <span className={`tag ${it.kind}`}>{it.kind === "ask" ? "✨ Ask AI" : "🎙 Dictated"}</span>
              <span className="when">{ago(it.at)}</span>
              <button
                className="btn tiny"
                onClick={async () => {
                  await api.historyCopy(it.text);
                  setCopied(it.at);
                  setTimeout(() => setCopied((c) => (c === it.at ? null : c)), 1600);
                }}
              >
                {copied === it.at ? "✓ Copied" : "Copy"}
              </button>
            </div>
            {it.asked && <p className="hist-asked">“{it.asked}”</p>}
            <p className="hist-text">{it.text}</p>
          </div>
        ))}
      </div>
      <button
        className="btn danger"
        onClick={async () => {
          await api.historyClear();
          load();
        }}
      >
        Clear history
      </button>
    </>
  );
}
