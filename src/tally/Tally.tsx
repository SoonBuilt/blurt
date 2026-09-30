import type { CSSProperties } from "react";

/**
 * Tally, soonbuilt's buddy: a sticky note with a face. Ported from TallyReel's
 * `components/tally/tally.tsx` (same geometry, moods and papers) so Tally looks
 * identical in every soonbuilt app.
 */
export type TallyMood = "hello" | "listening" | "pointing" | "working" | "ready" | "needs";

export const TALLY_PAPER = {
  peach: "#ffe2cc",
  butter: "#fff0b0",
  mint: "#d7f1e0",
  sky: "#dde8ff",
  lilac: "#ebe1ff",
} as const;
export type TallyPaper = keyof typeof TALLY_PAPER;

export const MOOD_PAPER: Record<TallyMood, TallyPaper> = {
  hello: "peach",
  listening: "butter",
  pointing: "sky",
  working: "sky",
  ready: "mint",
  needs: "peach",
};

const INK = "#1c1917";
const CHEEK = "rgba(240,106,29,.28)";

type Part = { l: number; t: number; w: number; h: number; css: CSSProperties };

/** Face parts on a 96px note, scaled to any size. */
function faceParts(mood: TallyMood, k: number): Part[] {
  const px = (v: number) => v * k;
  const openEyes = (dx = 0, dy = 0): Part[] => [
    { l: 25.9 + dx, t: 34.6 + dy, w: 10.6, h: 15.4, css: { borderRadius: px(5.8), background: INK } },
    { l: 54.7 + dx, t: 34.6 + dy, w: 10.6, h: 15.4, css: { borderRadius: px(5.8), background: INK } },
  ];
  const cheeks: Part[] = [
    { l: 12.5, t: 53.8, w: 11.5, h: 6.7, css: { borderRadius: "50%", background: CHEEK } },
    { l: 71, t: 53.8, w: 11.5, h: 6.7, css: { borderRadius: "50%", background: CHEEK } },
  ];
  const smile: Part = {
    l: 39.4,
    t: 55.7,
    w: 17.3,
    h: 8.6,
    css: { borderBottom: `${px(4.3)}px solid ${INK}`, borderRadius: `0 0 ${px(11.5)}px ${px(11.5)}px`, boxSizing: "border-box" },
  };
  const flat: Part = { l: 40.3, t: 63.4, w: 15.4, h: 3.8, css: { borderRadius: px(1.9), background: INK } };
  switch (mood) {
    case "listening":
      return [...openEyes(5.8, -7.7), ...cheeks, smile];
    case "working":
      return [
        { l: 25, t: 41.3, w: 14.4, h: 4.3, css: { borderRadius: px(2.9), background: INK } },
        { l: 53.8, t: 41.3, w: 14.4, h: 4.3, css: { borderRadius: px(2.9), background: INK } },
        ...cheeks,
        flat,
      ];
    case "ready": {
      const lid: CSSProperties = { borderTop: `${px(4.8)}px solid ${INK}`, borderRadius: `${px(11.5)}px ${px(11.5)}px 0 0`, boxSizing: "border-box" };
      return [
        { l: 25, t: 38.4, w: 14.4, h: 7.7, css: lid },
        { l: 53.8, t: 38.4, w: 14.4, h: 7.7, css: lid },
        ...cheeks,
        { l: 36.5, t: 55.7, w: 23, h: 12.5, css: { borderRadius: `0 0 ${px(13.4)}px ${px(13.4)}px`, background: INK } },
      ];
    }
    case "needs":
      return [...openEyes(), ...cheeks, flat];
    default:
      return [...openEyes(), ...cheeks, smile];
  }
}

export function Tally({
  size = 48,
  mood = "hello",
  paper,
  tilt = -4,
  tape,
  shadow = true,
  className = "",
  label,
}: {
  size?: number;
  mood?: TallyMood;
  paper?: TallyPaper;
  tilt?: number;
  tape?: boolean;
  shadow?: boolean;
  className?: string;
  label?: string;
}) {
  const k = size / 96;
  const px = (v: number) => v * k;
  const showTape = tape ?? size >= 40;
  return (
    <span
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-mood={mood}
      className={`tally ${className}`}
      style={{
        position: "relative",
        display: "inline-block",
        flex: "none",
        width: size,
        height: size,
        background: TALLY_PAPER[paper ?? MOOD_PAPER[mood]],
        borderRadius: `${px(6.7)}px ${px(6.7)}px ${px(6.7)}px ${px(25)}px`,
        boxShadow: shadow
          ? `0 1px 1px rgba(28,25,23,.06), 0 ${px(21.1)}px ${px(34.6)}px -${px(19.2)}px rgba(28,25,23,.4)`
          : "0 0 0 1px rgba(28,25,23,.08)",
        transform: `rotate(${tilt}deg)`,
      }}
    >
      {showTape ? (
        <span
          style={{
            position: "absolute",
            display: "block",
            left: px(26.9),
            top: px(-7.7),
            width: px(42.2),
            height: px(14.4),
            background: "rgba(255,255,255,.72)",
            boxShadow: "0 1px 2px rgba(0,0,0,.08)",
            transform: "rotate(-4deg)",
          }}
        />
      ) : null}
      {faceParts(mood, k).map((p, i) => (
        <span
          key={i}
          style={{ position: "absolute", display: "block", left: px(p.l), top: px(p.t), width: px(p.w), height: px(p.h), ...p.css }}
        />
      ))}
    </span>
  );
}
