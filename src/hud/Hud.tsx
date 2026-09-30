import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { TONE_LOOK, type Tone } from "../lib/tone";
import { Tally, type TallyMood, type TallyPaper } from "../tally/Tally";

type HudState =
  | { state: "hidden" }
  | { state: "listening"; ai: boolean; contextWords: number; handsFree: boolean }
  | { state: "transcribing"; ai: boolean }
  | { state: "thinking"; instruction: string; contextWords: number; tone: Tone | null }
  | { state: "done"; message: string; tone: Tone | null }
  | { state: "error"; message: string };

const BARS = 18;
const isMac = navigator.userAgent.includes("Mac");
const AI_KEY = isMac ? "⇧" : "Shift";

export default function Hud() {
  const [s, setS] = useState<HudState>({ state: "hidden" });
  const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0));
  const [seconds, setSeconds] = useState(0);
  const startedAt = useRef(0);

  useEffect(() => {
    const offState = listen<HudState>("hud", (e) => {
      setS((prev) => {
        if (e.payload.state === "listening" && prev.state !== "listening") {
          startedAt.current = Date.now();
          setSeconds(0);
          setLevels(Array(BARS).fill(0));
        }
        return e.payload;
      });
    });
    const offLevel = listen<number>("level", (e) => {
      setLevels((prev) => [...prev.slice(1), e.payload]);
    });
    return () => {
      offState.then((f) => f());
      offLevel.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (s.state !== "listening") return;
    const t = setInterval(() => setSeconds(Math.floor((Date.now() - startedAt.current) / 1000)), 250);
    return () => clearInterval(t);
  }, [s.state]);

  if (s.state === "hidden") return null;

  const ai = s.state === "listening" || s.state === "transcribing" ? s.ai : s.state === "thinking";
  let mood: TallyMood = "hello";
  let paper: TallyPaper | undefined;
  let anim = "";
  let body: React.ReactNode = null;

  switch (s.state) {
    case "listening":
      mood = "listening";
      paper = ai ? "lilac" : "butter";
      anim = "bob";
      body = (
        <>
          <Wave levels={levels} />
          <span className="lbl">{ai ? "Ask AI" : "Listening"}</span>
          {s.handsFree ? <span className="sub">hands-free · stops when you're done</span> : !ai && <span className="sub">+{AI_KEY} for AI</span>}
          <span className="sub mono">0:{String(seconds).padStart(2, "0")}</span>
        </>
      );
      break;
    case "transcribing":
      mood = "working";
      paper = ai ? "lilac" : "sky";
      anim = "wiggle";
      body = (
        <>
          <span className="lbl">{ai ? "Listening back…" : "Writing it down…"}</span>
          <span className="sub">on-device</span>
        </>
      );
      break;
    case "thinking":
      mood = "working";
      paper = "lilac";
      anim = "wiggle";
      body = (
        <>
          <span className="lbl">On it…</span>
          <span className="chip" title={s.instruction}>“{s.instruction}”</span>
          {s.contextWords > 0 && <span className="chip ctx">📎 {s.contextWords} words</span>}
          {s.tone && s.tone !== "calm" && (
            <span className="chip tone">
              {TONE_LOOK[s.tone].emoji} {TONE_LOOK[s.tone].doing}
            </span>
          )}
        </>
      );
      break;
    case "done": {
      const look = s.tone ? TONE_LOOK[s.tone] : null;
      mood = look ? look.mood : "ready";
      paper = look?.paper;
      anim = "pop";
      body = (
        <>
          <span className="lbl">✓ {s.message}</span>
          {look && (
            <span className="chip tone">
              {look.emoji} {look.heard.toLowerCase()}
              {s.tone !== "calm" && <span className="doing"> · {look.doing}</span>}
            </span>
          )}
        </>
      );
      break;
    }
    case "error":
      mood = "needs";
      anim = "pop";
      body = <span className="lbl err">{s.message}</span>;
      break;
  }

  return (
    <div className="wrap">
      <div className={`pill ${ai ? "ai" : ""} ${s.state}`} key={s.state === "done" || s.state === "error" ? s.state : "live"}>
        <span className={`slot ${anim}`}>
          <Tally size={30} mood={mood} paper={paper} tape={false} shadow={false} tilt={-5} />
        </span>
        {body}
      </div>
    </div>
  );
}

function Wave({ levels }: { levels: number[] }) {
  return (
    <span className="wave" aria-hidden>
      {levels.map((l, i) => (
        <i key={i} style={{ height: `${4 + Math.round(l * 18)}px` }} />
      ))}
    </span>
  );
}
