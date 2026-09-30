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
const GAP = 12;
const EDGE = 10;
const TALLY = 64;

type Box = { x: number; y: number; w: number; h: number };
type Spot = { tally: { x: number; y: number }; bubble: { x: number; y: number }; tail: "tl" | "tr" };

const hits = (a: Box, b: Box) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
const onScreen = (b: Box) => b.x >= EDGE && b.y >= EDGE && b.x + b.w <= window.innerWidth - EDGE && b.y + b.h <= window.innerHeight - EDGE;

/** The scrollable panel a target lives in, if any. */
function scroller(el: Element): HTMLElement | null {
  let p = el.parentElement;
  while (p) {
    const o = getComputedStyle(p).overflowY;
    if ((o === "auto" || o === "scroll") && p.scrollHeight > p.clientHeight + 4) return p;
    p = p.parentElement;
  }
  return null;
}

/** Scrolls the target into the upper part of its panel so Tally has room underneath. */
function makeRoom(el: Element) {
  const sc = scroller(el);
  if (!sc) return;
  const want = sc.getBoundingClientRect().top + 24;
  const by = el.getBoundingClientRect().top - want;
  // Only scroll when the target sits low enough that the bubble wouldn't fit below it.
  if (by > 40) sc.scrollTop += by;
}

/**
 * Where Tally and its bubble can stand: below the target, above it, or beside it.
 * The first spot that fits on screen **and leaves the target completely uncovered**
 * wins, so Tally never sits on the very thing it's asking you to use.
 */
function findSpot(target: Box, bw: number, bh: number): Spot | null {
  // Stay beside the target rather than drifting over the sidebar.
  const clampX = (x: number) => Math.min(Math.max(Math.min(target.x, window.innerWidth - bw - TALLY - 34), x), window.innerWidth - EDGE - bw - TALLY - 10);
  const pairs = (tx: number, ty: number): Spot[] => [
    { tally: { x: tx, y: ty }, bubble: { x: tx + TALLY + 10, y: ty }, tail: "tl" },
    { tally: { x: tx + bw + 10, y: ty }, bubble: { x: tx, y: ty }, tail: "tr" },
  ];
  const blockH = Math.max(TALLY, bh);
  const candidates: Spot[] = [
    // below the target, then above it: the natural reading order
    ...pairs(clampX(target.x + 8), target.y + target.h + GAP),
    ...pairs(clampX(target.x + 8), target.y - GAP - blockH),
    // beside it, when there's room to the right or left
    ...pairs(target.x + target.w + GAP, Math.max(GAP, target.y)),
    ...pairs(target.x - GAP - bw - TALLY - 10, Math.max(GAP, target.y)),
  ];
  for (const c of candidates) {
    const block: Box = {
      x: Math.min(c.tally.x, c.bubble.x),
      y: Math.min(c.tally.y, c.bubble.y),
      w: bw + TALLY + 10,
      h: blockH,
    };
    if (onScreen(block) && !hits(block, { ...target, x: target.x - 6, y: target.y - 6, w: target.w + 12, h: target.h + 12 })) {
      return c;
    }
  }
  return null;
}

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
  const [spot, setSpot] = useState<Spot | null>(null);
  const [hopping, setHopping] = useState(false);
  const [celebrate, setCelebrate] = useState(false);
  const step = list[Math.min(i, list.length - 1)];
  const isDone = step.done ? step.done(status, practiced) : false;
  const glowRef = useRef<Element | null>(null);
  const bubbleRef = useRef<HTMLDivElement | null>(null);

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
    const corner = (): Spot => {
      const bw = bubbleRef.current?.offsetWidth ?? 300;
      const bh = bubbleRef.current?.offsetHeight ?? 170;
      const y = window.innerHeight - GAP - Math.max(TALLY, bh);
      return { tally: { x: window.innerWidth - GAP - TALLY, y }, bubble: { x: window.innerWidth - GAP - TALLY - 10 - bw, y }, tail: "tr" };
    };
    const place = () => {
      const el = step.target ? document.querySelector(step.target) : null;
      if (glowRef.current !== el) {
        glowRef.current?.classList.remove("tour-glow");
        glowRef.current = el;
        el?.classList.add("tour-glow");
      }
      if (!el) {
        setSpot(corner());
        return;
      }
      const r = el.getBoundingClientRect();
      const bw = bubbleRef.current?.offsetWidth ?? (step.family ? 360 : 300);
      const bh = bubbleRef.current?.offsetHeight ?? 170;
      setSpot(findSpot({ x: r.left, y: r.top, w: r.width, h: r.height }, bw, bh) ?? corner());
    };
    setHopping(true);
    // Bring the target into view first, then measure where everything ended up.
    const t = step.target ? document.querySelector(step.target) : null;
    if (t) {
      t.scrollIntoView({ block: "nearest" });
      makeRoom(t);
    }
    const t1 = setTimeout(place, 60);
    const t2 = setTimeout(() => setHopping(false), HOP_MS + 80);
    window.addEventListener("resize", place);
    // The settings panel scrolls; follow the target when it moves.
    window.addEventListener("scroll", place, true);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [step.id, step.target, step.family]);

  useEffect(() => () => glowRef.current?.classList.remove("tour-glow"), []);

  // When the user finishes a step by themselves, cheer and move on.
  useEffect(() => {
    if (!isDone) return;
    const t = setTimeout(next, 1500);
    return () => clearTimeout(t);
  }, [isDone, next]);

  const mood: TallyMood = celebrate || isDone ? "ready" : step.mood;

  return (
    <div className="tour" aria-live="polite">
      <div
        className={`tour-tally ${hopping ? "hop" : "idle"} ${celebrate ? "cheer" : ""} ${spot ? "" : "unplaced"}`}
        style={spot ? { transform: `translate(${spot.tally.x}px, ${spot.tally.y}px)` } : undefined}
      >
        <Tally size={64} mood={mood} paper={isDone || celebrate ? "mint" : step.paper} tilt={-6} label="Tally, your guide" />
      </div>
      <div
        ref={bubbleRef}
        className={`tour-bubble tail-${spot?.tail ?? "tl"} ${step.family ? "wide" : ""} ${spot ? "" : "unplaced"}`}
        style={spot ? { transform: `translate(${spot.bubble.x}px, ${spot.bubble.y}px)` } : undefined}
      >
        <div className="tour-head">
          <b>{isDone ? "✓ " + step.title : step.title}</b>
          <span className="tour-count">
            {i + 1}/{list.length}
          </span>
        </div>
        <p key={step.id}>{isDone && step.doneSay ? step.doneSay : step.say}</p>
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
