import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, keyLabels, type Status } from "../lib/api";
import { Tally, type TallyMood, type TallyPaper } from "../tally/Tally";
import { FamilyCards } from "./Family";
import type { Page } from "./Settings";

/**
 * Tally's tour. It never blocks: the app stays fully usable while Tally hops from one
 * thing to the next with a small speech bubble. Steps the user finishes by themselves
 * (granting a permission, downloading the model, trying dictation) move the tour along.
 */

type Step = {
  id: string;
  page?: Page;
  /** CSS selector for the thing Tally points at; none = Tally floats in the corner. */
  target?: string;
  mood: TallyMood;
  paper?: TallyPaper;
  title: string;
  say: React.ReactNode;
  /** When true the step is done without pressing Next. */
  done?: (s: Status, practiced: boolean) => boolean;
  doneSay?: string;
  family?: boolean;
};

function steps(status: Status): Step[] {
  const k = keyLabels(status.platform);
  const mac = status.platform === "macos";
  return [
    {
      id: "hello",
      mood: "hello",
      title: "Hi, I'm Tally!",
      say: <>Blurt is all yours already. Want a one-minute tour? I'll hop around and show you the good bits.</>,
    },
    ...(mac
      ? [
          {
            id: "perms",
            page: "voice" as Page,
            target: '[data-tour="perms"]',
            mood: "needs" as TallyMood,
            title: "Two quick permissions",
            say: <>So I can hear you while you hold the key, and type into the app you're using. Allow both here.</>,
            done: (s: Status) => s.accessibility && s.microphone === "allowed",
            doneSay: "Perfect, that's all I need!",
          },
        ]
      : []),
    {
      id: "model",
      page: "voice",
      target: '[data-tour="model"]',
      mood: "pointing",
      paper: "sky",
      title: "Give me my ears",
      say: <>This is the voice model. It downloads once and your voice never leaves your {mac ? "Mac" : "PC"}.</>,
      done: (s) => s.modelDownloaded,
      doneSay: "Got it! I can hear you now.",
    },
    {
      id: "practice",
      page: "general",
      target: '[data-tour="practice"]',
      mood: "listening",
      paper: "butter",
      title: "Try it",
      say: (
        <>
          Click this box, hold <kbd>{k.talk}</kbd>, say something and let go.
        </>
      ),
      done: (_s, practiced) => practiced,
      doneSay: "See? That was me!",
    },
    {
      id: "ai",
      page: "general",
      target: '[data-tour="practice"]',
      mood: "pointing",
      paper: "lilac",
      title: "Now the magic trick",
      say: (
        <>
          Highlight what you wrote, hold <kbd>{k.talk}</kbd> + <kbd>{k.ai}</kbd> and say “make it more fun”. Or ask me anything with nothing highlighted.
        </>
      ),
    },
    {
      id: "style",
      page: "style",
      target: '[data-tour="nav-style"]',
      mood: "pointing",
      paper: "butter",
      title: "Sound like you",
      say: <>Answer a few quick questions here and everything I write will sound like you. All optional.</>,
    },
    {
      id: "smarts",
      page: "smarts",
      target: '[data-tour="nav-smarts"]',
      mood: "ready",
      title: "A few smarts",
      say: (
        <>
          Double-tap <kbd>{k.talk}</kbd> to talk hands-free. I remember your last few asks for 10 minutes, and I can notice your tone.
        </>
      ),
    },
    {
      id: "family",
      mood: "hello",
      paper: "peach",
      title: "One more thing!",
      say: <>I live in the other soonbuilt apps too. Come say hi sometime.</>,
      family: true,
    },
  ];
}

const HOP_MS = 650;

export default function Tour({
  status,
  setPage,
  practiced,
  onClose,
}: {
  status: Status;
  setPage: (p: Page) => void;
  practiced: boolean;
  onClose: (finished: boolean) => void;
}) {
  const list = steps(status);
  const [i, setI] = useState(0);
  const [pos, setPos] = useState<{ x: number; y: number; side: "left" | "right" }>(() => ({ x: window.innerWidth - 120, y: window.innerHeight - 170, side: "left" }));
  const [hopping, setHopping] = useState(false);
  const [celebrate, setCelebrate] = useState(false);
  const step = list[Math.min(i, list.length - 1)];
  const isDone = step.done ? step.done(status, practiced) : false;
  const glowRef = useRef<Element | null>(null);

  const next = useCallback(() => {
    if (i >= list.length - 1) {
      setCelebrate(true);
      setTimeout(() => onClose(true), 900);
    } else {
      setI(i + 1);
    }
  }, [i, list.length, onClose]);

  // Move to the step's page, then hop next to its target.
  useEffect(() => {
    if (step.page) setPage(step.page);
  }, [step.id, step.page, setPage]);

  useLayoutEffect(() => {
    const place = () => {
      glowRef.current?.classList.remove("tour-glow");
      const el = step.target ? document.querySelector(step.target) : null;
      if (el) {
        el.classList.add("tour-glow");
        glowRef.current = el;
        const r = el.getBoundingClientRect();
        const roomRight = window.innerWidth - r.right > 360;
        const x = roomRight ? r.right + 18 : Math.max(340, r.left + Math.min(r.width, 420) - 70);
        const y = roomRight ? r.top + Math.min(r.height / 2, 80) - 36 : Math.min(r.bottom + 12, window.innerHeight - 190);
        setPos({ x, y: Math.max(16, y), side: roomRight ? "right" : "left" });
      } else {
        setPos({ x: window.innerWidth - 120, y: window.innerHeight - 170, side: "left" });
      }
    };
    setHopping(true);
    const t1 = setTimeout(place, 60); // let the page render first
    const t2 = setTimeout(() => setHopping(false), HOP_MS + 80);
    window.addEventListener("resize", place);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      window.removeEventListener("resize", place);
    };
  }, [step.id, step.target]);

  useEffect(() => () => glowRef.current?.classList.remove("tour-glow"), []);

  // When the user finishes a step by themselves, cheer and move on.
  useEffect(() => {
    if (!isDone) return;
    const t = setTimeout(next, 1500);
    return () => clearTimeout(t);
  }, [isDone, next]);

  const mood: TallyMood = celebrate || isDone ? "ready" : step.mood;
  const bubbleLeft = pos.side === "right";

  return (
    <div className="tour" aria-live="polite">
      <div
        className={`tour-tally ${hopping ? "hop" : "idle"} ${celebrate ? "cheer" : ""}`}
        style={{ transform: `translate(${pos.x}px, ${pos.y}px)` }}
      >
        <Tally size={64} mood={mood} paper={isDone || celebrate ? "mint" : step.paper} tilt={-6} label="Tally, your guide" />
        <div className={`tour-bubble ${bubbleLeft ? "to-right" : "to-left"} ${step.family ? "wide" : ""}`} key={step.id}>
          <div className="tour-head">
            <b>{isDone ? "✓ " + step.title : step.title}</b>
            <span className="tour-count">
              {i + 1}/{list.length}
            </span>
          </div>
          <p>{isDone && step.doneSay ? step.doneSay : step.say}</p>
          {step.family && <FamilyCards compact />}
          <div className="tour-actions">
            {step.id === "hello" ? (
              <>
                <button className="btn pri" onClick={next}>
                  Show me around
                </button>
                <button className="btn" onClick={() => onClose(true)}>
                  Maybe later
                </button>
              </>
            ) : (
              <>
                <button className="btn pri" onClick={next}>
                  {i === list.length - 1 ? "Finish" : isDone ? "Next →" : step.done ? "Skip this →" : "Next →"}
                </button>
                {i > 0 && i < list.length - 1 && (
                  <button className="btn ghost" onClick={() => onClose(true)}>
                    End tour
                  </button>
                )}
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

/** Marks the tour as seen so it doesn't start again. */
export async function markToured() {
  try {
    await api.finishOnboarding();
  } catch {
    /* browser preview */
  }
}
