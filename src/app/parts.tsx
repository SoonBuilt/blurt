import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, type AiProvider, type Pack, type Settings, type Status, type WritingProfile } from "../lib/api";
import { ago, TONE_LOOK } from "../lib/tone";
import { Tally } from "../tally/Tally";

export function useSettings(status: Status, refresh: () => void) {
  const save = async (patch: (s: Settings) => Settings) => {
    await api.saveSettings(patch(structuredClone(status.settings)));
    refresh();
  };
  return save;
}

export function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button className={`tog ${on ? "on" : ""}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />
  );
}

/* ---------------- Permissions ---------------- */

export function Permissions({ status }: { status: Status }) {
  const mic = status.microphone;
  const [asking, setAsking] = useState(false);
  return (
    <div className="stack">
      <div className={`perm ${mic === "allowed" ? "granted" : ""}`}>
        <div className="pi">🎙</div>
        <div className="grow">
          <b>Microphone</b>
          <span>So Tally can hear you while you hold the key. Never in the background.</span>
        </div>
        {mic === "allowed" ? (
          <span className="ok">✓ Allowed</span>
        ) : mic === "unknown" ? (
          <button
            className="btn acc"
            disabled={asking}
            onClick={async () => {
              setAsking(true);
              await api.requestMicrophone();
              setAsking(false);
            }}
          >
            Allow
          </button>
        ) : (
          <button className="btn" onClick={() => api.openPrivacySettings("microphone")}>
            Open Settings
          </button>
        )}
      </div>
      <div className={`perm ${status.accessibility ? "granted" : ""}`}>
        <div className="pi">⌨️</div>
        <div className="grow">
          <b>Accessibility</b>
          <span>To notice the talk key, read what you've highlighted and type into the app you're using.</span>
        </div>
        {status.accessibility ? (
          <span className="ok">✓ Allowed</span>
        ) : (
          <button className="btn acc" onClick={() => api.requestAccessibility()}>
            Open Settings
          </button>
        )}
      </div>
      {!status.accessibility && (
        <p className="hint">
          In System Settings, switch on <b>Blurt</b> under Privacy & Security → Accessibility. Tally will notice as soon as you do.
        </p>
      )}
    </div>
  );
}

/* ---------------- Downloadable packs ---------------- */

export function PackCard({
  pack,
  title,
  blurb,
  size,
  ready,
  refresh,
  children,
}: {
  pack: Pack;
  title: string;
  blurb: string;
  size: string;
  ready: boolean;
  refresh: () => void;
  children?: React.ReactNode;
}) {
  const [progress, setProgress] = useState<[number, number] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    const off = listen<[Pack, number, number]>("pack-progress", (e) => {
      if (e.payload[0] === pack) setProgress([e.payload[1], e.payload[2]]);
    });
    return () => {
      off.then((f) => f());
    };
  }, [pack]);

  const start = async () => {
    setError("");
    setProgress([0, 1]);
    try {
      await api.downloadPack(pack);
      refresh();
    } catch (e) {
      setError(String(e));
      setProgress(null);
    }
  };

  const pct = progress ? Math.floor((progress[0] / Math.max(progress[1], 1)) * 100) : 0;
  const mb = (b: number) => Math.round(b / 1_000_000);

  return (
    <div className="card model">
      <div className="row-flex">
        <div className="grow">
          <b>{title}</b>
          <span>{blurb}</span>
        </div>
        <em className="mono faint">{size}</em>
      </div>
      {ready ? (
        <div className="row-flex ready-row">
          <p className="ok grow">✓ Installed and ready</p>
          {children}
        </div>
      ) : progress ? (
        <>
          <div className="bar">
            <i style={{ width: `${pct}%` }} />
          </div>
          <div className="bar-lbl mono">
            <span>
              {mb(progress[0])} / {mb(progress[1])} MB
            </span>
            <span>{pct}%</span>
          </div>
        </>
      ) : (
        <button className="btn pri" onClick={start}>
          Download
        </button>
      )}
      {error && <p className="err">{error}</p>}
    </div>
  );
}

export function VoiceModel({ status, refresh }: { status: Status; refresh: () => void }) {
  return (
    <PackCard
      pack="speech"
      title="Parakeet voice model"
      blurb={`Fast, accurate speech recognition that runs on your ${status.platform === "macos" ? "Mac" : "PC"}. English plus 24 European languages.`}
      size="680 MB"
      ready={status.modelDownloaded}
      refresh={refresh}
    />
  );
}

export function ToneAwareness({ status, refresh }: { status: Status; refresh: () => void }) {
  return (
    <PackCard
      pack="tone"
      title="Tone awareness"
      blurb="Tally hears whether you sound stressed, annoyed or upbeat, and answers accordingly. Your audio never leaves your computer."
      size="373 MB"
      ready={status.toneReady}
      refresh={refresh}
    />
  );
}

/* ---------------- AI engine ---------------- */

const ENGINES: { id: AiProvider; name: string; blurb: string; mac?: boolean }[] = [
  { id: "apple", name: "Apple Intelligence", blurb: "Free and private, right on your Mac. Nothing to set up.", mac: true },
  { id: "ollama", name: "Ollama", blurb: "Free and private. Runs an open model on your computer." },
  { id: "anthropic", name: "Claude", blurb: "Your own Anthropic API key. The smartest option; you pay Anthropic." },
  { id: "openai", name: "OpenAI-compatible", blurb: "Your own key for OpenAI, OpenRouter, Groq or any compatible API." },
];

export function AiEngine({ status, refresh }: { status: Status; refresh: () => void }) {
  const save = useSettings(status, refresh);
  const ai = status.settings.ai;
  const [key, setKey] = useState("");
  const [test, setTest] = useState<{ state: "idle" | "busy" | "ok" | "err"; text: string }>({ state: "idle", text: "" });
  const engines = ENGINES.filter((e) => !e.mac || status.platform === "macos");

  const runTest = async () => {
    setTest({ state: "busy", text: "" });
    try {
      setTest({ state: "ok", text: await api.testAi() });
    } catch (e) {
      setTest({ state: "err", text: String(e) });
    }
  };

  const hasKey = ai.provider === "anthropic" ? status.hasAnthropicKey : status.hasOpenaiKey;

  return (
    <div className="stack">
      <div className="engines">
        {engines.map((e) => (
          <button
            key={e.id}
            className={`engine ${ai.provider === e.id ? "on" : ""}`}
            onClick={() => {
              setTest({ state: "idle", text: "" });
              save((s) => ({ ...s, ai: { ...s.ai, provider: e.id } }));
            }}
          >
            <b>{e.name}</b>
            <span>{e.blurb}</span>
          </button>
        ))}
      </div>

      {ai.provider === "apple" && status.appleAi && (
        <div className="warn">
          <Tally size={26} mood="needs" tape={false} shadow={false} />
          <span className="grow">{status.appleAi}</span>
          <button className="btn" onClick={() => api.openPrivacySettings("ai")}>
            Open Settings
          </button>
        </div>
      )}

      {ai.provider === "ollama" && (
        <div className="group">
          <label className="row">
            <span className="t">
              <b>Model</b>
              <span>
                Any model you've pulled, e.g. <code>ollama pull llama3.2:3b</code>
              </span>
            </span>
            <input className="inp" defaultValue={ai.ollama_model} onBlur={(e) => save((s) => ({ ...s, ai: { ...s.ai, ollama_model: e.target.value.trim() } }))} />
          </label>
          <label className="row">
            <span className="t">
              <b>Address</b>
            </span>
            <input className="inp" defaultValue={ai.ollama_url} onBlur={(e) => save((s) => ({ ...s, ai: { ...s.ai, ollama_url: e.target.value.trim() } }))} />
          </label>
        </div>
      )}

      {(ai.provider === "anthropic" || ai.provider === "openai") && (
        <div className="group">
          <label className="row">
            <span className="t">
              <b>API key</b>
              <span>{hasKey ? "Saved in your keychain. Paste a new one to replace it." : "Stored in your system keychain, never in a file."}</span>
            </span>
            <input
              className="inp"
              type="password"
              placeholder={hasKey ? "••••••••••••" : ai.provider === "anthropic" ? "sk-ant-…" : "sk-…"}
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onBlur={async () => {
                if (key.trim()) {
                  await api.setApiKey(ai.provider, key);
                  setKey("");
                  refresh();
                }
              }}
            />
          </label>
          {ai.provider === "anthropic" ? (
            <label className="row">
              <span className="t">
                <b>Model</b>
              </span>
              <input className="inp" defaultValue={ai.anthropic_model} onBlur={(e) => save((s) => ({ ...s, ai: { ...s.ai, anthropic_model: e.target.value.trim() } }))} />
            </label>
          ) : (
            <>
              <label className="row">
                <span className="t">
                  <b>Model</b>
                </span>
                <input className="inp" defaultValue={ai.openai_model} onBlur={(e) => save((s) => ({ ...s, ai: { ...s.ai, openai_model: e.target.value.trim() } }))} />
              </label>
              <label className="row">
                <span className="t">
                  <b>Base URL</b>
                </span>
                <input className="inp" defaultValue={ai.openai_url} onBlur={(e) => save((s) => ({ ...s, ai: { ...s.ai, openai_url: e.target.value.trim() } }))} />
              </label>
            </>
          )}
        </div>
      )}

      <div className="test">
        <button className="btn" onClick={runTest} disabled={test.state === "busy"}>
          {test.state === "busy" ? "Asking Tally…" : "Test it"}
        </button>
        {test.state === "ok" && (
          <span className="said">
            <Tally size={22} mood="ready" tape={false} shadow={false} /> {test.text}
          </span>
        )}
        {test.state === "err" && <span className="err">{test.text}</span>}
      </div>
    </div>
  );
}

/* ---------------- How you write ---------------- */

type Choice<T extends string> = { v: T; l: string };

function Chips<T extends string>({ value, options, onPick }: { value: T; options: Choice<T>[]; onPick: (v: T) => void }) {
  return (
    <div className="chips" role="radiogroup">
      {options.map((o) => (
        <button key={o.v} role="radio" aria-checked={value === o.v} className={value === o.v ? "on" : ""} onClick={() => onPick(value === o.v ? ("" as T) : o.v)}>
          {o.l}
        </button>
      ))}
    </div>
  );
}

/** "How do you write?" A short questionnaire that shapes every AI answer. */
export function WritingStyle({ status, refresh }: { status: Status; refresh: () => void }) {
  const save = useSettings(status, refresh);
  const p = status.settings.profile;
  const set = <K extends keyof WritingProfile>(k: K, v: WritingProfile[K]) => save((x) => ({ ...x, profile: { ...x.profile, [k]: v } }));

  return (
    <div className="quiz">
      <div className="q two">
        <label>
          <b>What should Tally call you?</b>
          <input className="inp" placeholder="e.g. Danish" defaultValue={p.name} onBlur={(e) => set("name", e.target.value.trim())} />
        </label>
        <label>
          <b>What do you do?</b>
          <input className="inp" placeholder="e.g. product designer at a small studio" defaultValue={p.role} onBlur={(e) => set("role", e.target.value.trim())} />
        </label>
      </div>
      <div className="q">
        <b>How do you usually sound?</b>
        <Chips
          value={p.tone}
          onPick={(v) => set("tone", v)}
          options={[
            { v: "casual", l: "😎 Casual" },
            { v: "friendly", l: "😊 Friendly" },
            { v: "professional", l: "💼 Professional" },
            { v: "formal", l: "🎩 Formal" },
          ]}
        />
      </div>
      <div className="q">
        <b>How long should things be?</b>
        <Chips
          value={p.length}
          onPick={(v) => set("length", v)}
          options={[
            { v: "brief", l: "Short and sweet" },
            { v: "balanced", l: "Balanced" },
            { v: "detailed", l: "Thorough" },
          ]}
        />
      </div>
      <div className="q two">
        <div>
          <b>Spelling</b>
          <Chips
            value={p.spelling}
            onPick={(v) => set("spelling", v)}
            options={[
              { v: "us", l: "🇺🇸 American" },
              { v: "uk", l: "🇬🇧 British" },
            ]}
          />
        </div>
        <div>
          <b>Emojis</b>
          <Chips
            value={p.emoji}
            onPick={(v) => set("emoji", v)}
            options={[
              { v: "never", l: "Never" },
              { v: "sometimes", l: "Sometimes" },
              { v: "often", l: "Love them" },
            ]}
          />
        </div>
      </div>
      <label className="q">
        <b>How do you sign off emails?</b>
        <input className="inp wide" placeholder="e.g. Cheers, Dan" defaultValue={p.sign_off} onBlur={(e) => set("sign_off", e.target.value.trim())} />
      </label>
      <label className="q">
        <b>Anything else Tally should know?</b>
        <span className="hint">Your company, people you often write to, words you love or hate, how to spell your product names…</span>
        <textarea
          className="style"
          placeholder="e.g. I run soonbuilt, a small product studio. Sara is our designer. Never say “synergy”."
          defaultValue={p.extra}
          onBlur={(e) => set("extra", e.target.value.trim())}
        />
      </label>
    </div>
  );
}

/* ---------------- How you've sounded ---------------- */

export function ToneReading({ status }: { status: Status }) {
  const recent = status.recentTones;
  const latest = recent[0];
  const look = latest ? TONE_LOOK[latest[0]] : null;
  return (
    <div className="card tone-card">
      <div className="row-flex">
        <Tally size={52} mood={look?.mood ?? "hello"} paper={look?.paper ?? "peach"} tilt={-5} />
        <div className="grow">
          <b>{look ? `Last time you sounded: ${look.heard.toLowerCase()} ${look.emoji}` : "Tally hasn't heard you yet"}</b>
          <span>
            {latest
              ? `${ago(latest[1])}. Tally shows this after you speak, and Ask AI adjusts: ${look?.doing}.`
              : "Once the tone download is in and you talk to Tally, your mood shows up here and after each recording."}
          </span>
        </div>
      </div>
      {recent.length > 1 && (
        <div className="tone-strip" aria-label="Recent tones, newest first">
          {recent.map(([t, s], i) => (
            <span key={i} className="tone-dot" title={`${TONE_LOOK[t].heard} · ${ago(s)}`}>
              <Tally size={26} mood={TONE_LOOK[t].mood} paper={TONE_LOOK[t].paper} tape={false} shadow={false} tilt={i % 2 ? 4 : -4} />
              <small>{TONE_LOOK[t].heard}</small>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
