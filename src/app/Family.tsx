import { openUrl } from "@tauri-apps/plugin-opener";
import { isTauri } from "@tauri-apps/api/core";
import { Tally, type TallyMood, type TallyPaper } from "../tally/Tally";

/** The other soonbuilt apps Tally lives in. Lines match soonbuilt.com (src/lib/site.ts). */
export const FAMILY: { name: string; line: string; kind: string; url: string; mood: TallyMood; paper: TallyPaper }[] = [
  {
    name: "TallyReel Studio",
    line: "Write one line. Get a finished short video, made on your own computer.",
    kind: "Mac + Windows",
    url: "https://tallyreel.com",
    mood: "working",
    paper: "butter",
  },
  {
    name: "TallyReel Post",
    line: "Pin a video to the week. It posts itself, to 15 networks.",
    kind: "Web + iOS + Android",
    url: "https://app.tallyreel.com",
    mood: "ready",
    paper: "mint",
  },
  {
    name: "Round Zero",
    line: "Mock job interviews with a live AI interviewer, by voice.",
    kind: "Web",
    url: "https://joinroundzero.com",
    mood: "listening",
    paper: "sky",
  },
];

export function open(url: string) {
  const tagged = `${url}${url.includes("?") ? "&" : "?"}ref=blurt`;
  if (isTauri()) openUrl(tagged);
  else window.open(tagged, "_blank", "noopener");
}

export function FamilyCards({ compact = false }: { compact?: boolean }) {
  return (
    <div className={`family ${compact ? "compact" : ""}`}>
      {FAMILY.map((f, i) => (
        <button key={f.name} className="fam" onClick={() => open(f.url)}>
          <Tally size={compact ? 30 : 40} mood={f.mood} paper={f.paper} tape={false} tilt={i % 2 ? 4 : -4} />
          <span className="grow">
            <b>{f.name}</b>
            <span>{f.line}</span>
            {!compact && <em>{f.kind}</em>}
          </span>
          <span className="go" aria-hidden>
            ↗
          </span>
        </button>
      ))}
    </div>
  );
}
