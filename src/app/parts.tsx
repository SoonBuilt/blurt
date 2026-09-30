import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, type AiProvider, type Pack, type Settings, type Status } from "../lib/api";
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

export function TallyVoice({ status, refresh }: { status: Status; refresh: () => void }) {
  const [playing, setPlaying] = useState(false);
  return (
    <PackCard
      pack="voice"
      title="Tally's voice"
      blurb="Tally answers your questions out loud, in a natural voice made on your computer."
      size="98 MB"
      ready={status.voiceReady}
      refresh={refresh}
    >
      <button
        className="btn"
        disabled={playing}
        onClick={async () => {
          setPlaying(true);
          try {
            await api.speakSample();
          } finally {
            setTimeout(() => setPlaying(false), 4000);
          }
        }}
      >
        {playing ? "🔊 Speaking…" : "▶ Hear Tally"}
      </button>
    </PackCard>
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

export function PremiumVoice({ status, refresh }: { status: Status; refresh: () => void }) {
  const save = useSettings(status, refresh);
  const on = status.settings.voice.premium_voice;
  return (
    <PackCard
      pack="premium"
      title="Expressive voice (Pro preview)"
      blurb="A richer, more human Tally that can laugh, chuckle and sigh (Chatterbox Turbo). English only, and it takes a moment longer to start talking."
      size="470 MB"
      ready={status.premiumReady}
      refresh={refresh}
    >
      <span className="pro">PRO</span>
      <Toggle label="Use the expressive voice" on={on} onChange={(v) => save((x) => ({ ...x, voice: { ...x.voice, premium_voice: v } }))} />
    </PackCard>
  );
}
