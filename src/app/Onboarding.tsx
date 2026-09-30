import { useState } from "react";
import { api, keyLabels, type Status } from "../lib/api";
import { Tally, type TallyMood, type TallyPaper } from "../tally/Tally";
import { AiEngine, Permissions, TallyVoice, VoiceModel } from "./parts";

type Step = { id: string; title: string };

export default function Onboarding({ status, refresh, onDone }: { status: Status; refresh: () => void; onDone: () => void }) {
  const mac = status.platform === "macos";
  const keys = keyLabels(status.platform);
  const steps: Step[] = [
    { id: "welcome", title: "Welcome" },
    ...(mac ? [{ id: "perms", title: "Permissions" }] : []),
    { id: "voice", title: "Voice model" },
    { id: "ai", title: "AI engine" },
    { id: "try", title: "Try it" },
  ];
  const [i, setI] = useState(0);
  const step = steps[i].id;
  const permsDone = status.microphone === "allowed" && status.accessibility;

  const buddy: Record<string, { mood: TallyMood; paper?: TallyPaper; say: React.ReactNode; anim?: string }> = {
    welcome: { mood: "hello", anim: "wave", say: <>Hi! I'm <b>Tally</b>. I live in your {mac ? "menu bar" : "system tray"} and turn what you say into writing.</> },
    perms: permsDone
      ? { mood: "ready", say: "Perfect, that's all I need!", anim: "pop" }
      : { mood: "needs", say: "I need two things before I can help." },
    voice: status.modelDownloaded
      ? { mood: "ready", say: "Got it! I can hear you now.", anim: "pop" }
      : { mood: "pointing", paper: "sky", say: "This is how I learn to hear you. It stays on your computer." },
    ai: { mood: "pointing", paper: "lilac", say: <>Add <b>{keys.ai}</b> while you talk and I'll use AI. Pick who does the thinking.</> },
    try: { mood: "listening", paper: "butter", anim: "bob", say: "Go on, say something. I'm listening." },
  };
  const b = buddy[step];

  const canContinue = step === "perms" ? permsDone : step === "voice" ? status.modelDownloaded : true;

  return (
    <div className="ob">
      <aside className="ob-side">
        <div className="brand">
          <Tally size={26} mood="hello" tape={false} shadow={false} />
          Blurt
        </div>
        <ol className="ob-steps">
          {steps.map((s, j) => (
            <li key={s.id} className={j === i ? "on" : j < i ? "done" : ""}>
              <span className="n">{j < i ? "✓" : j + 1}</span>
              {s.title}
            </li>
          ))}
        </ol>
        <p className="made">Made by soonbuilt</p>
      </aside>

      <main className="ob-main">
        <div className="buddy">
          <span className={b.anim ?? ""} key={step + b.mood}>
            <Tally size={step === "welcome" ? 96 : 64} mood={b.mood} paper={b.paper} tilt={-5} />
          </span>
          <div className="bubble">{b.say}</div>
        </div>

        {step === "welcome" && (
          <>
            <h1>Talk instead of typing.</h1>
            <p className="lead">
              Hold <kbd>{keys.talkLong}</kbd> anywhere and talk. Clean text appears wherever your cursor is. Add <kbd>{keys.aiLong}</kbd> and your voice becomes
              a prompt for AI, with anything you've highlighted as context.
            </p>
            <div className="pills">
              <span>🎙 Dictate anywhere</span>
              <span>✨ Ask AI by voice</span>
              <span>📎 Highlight = context</span>
              <span>🔒 Voice stays on your {mac ? "Mac" : "PC"}</span>
            </div>
          </>
        )}

        {step === "perms" && (
          <>
            <h1>Two quick permissions</h1>
            <p className="lead">macOS needs to know Blurt can hear you and type into other apps. You can change these any time in System Settings.</p>
            <Permissions status={status} />
          </>
        )}

        {step === "voice" && (
          <>
            <h1>Download the voice model</h1>
            <p className="lead">It's downloaded once and your voice never leaves your {mac ? "Mac" : "PC"}.</p>
            <VoiceModel status={status} refresh={refresh} />
          </>
        )}

        {step === "ai" && (
          <>
            <h1>Who should do the thinking?</h1>
            <p className="lead">Used when you hold {keys.aiLong} too. Dictation works without it, so you can set this up later.</p>
            <AiEngine status={status} refresh={refresh} />
            <h3>Want Tally to talk back?</h3>
            <TallyVoice status={status} refresh={refresh} />
          </>
        )}

        {step === "try" && (
          <>
            <h1>Try it now</h1>
            <p className="lead">
              Click the box, hold <kbd>{keys.talkLong}</kbd>, say something and let go. Then highlight what you wrote, hold <kbd>{keys.talk}</kbd> + <kbd>{keys.ai}</kbd> and
              say “make this sound more excited”.
            </p>
            <textarea className="tryit" placeholder={`Hold ${keys.talk} and say: “Hey, this is my first message with Blurt.”`} autoFocus />
          </>
        )}

        <footer className="ob-foot">
          {i > 0 ? (
            <button className="big ghost" onClick={() => setI(i - 1)}>
              ← Back
            </button>
          ) : (
            <span />
          )}
          {i < steps.length - 1 ? (
            <button className="big pri" disabled={!canContinue} onClick={() => setI(i + 1)}>
              {step === "welcome" ? "Get started →" : step === "ai" ? "Continue →" : "Continue →"}
            </button>
          ) : (
            <button
              className="big pri"
              onClick={async () => {
                await api.finishOnboarding();
                onDone();
              }}
            >
              Done. Tally's in your {mac ? "menu bar" : "tray"}
            </button>
          )}
        </footer>
      </main>
    </div>
  );
}
