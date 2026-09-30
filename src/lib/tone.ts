import type { TallyMood, TallyPaper } from "../tally/Tally";

export type Tone = "calm" | "frustrated" | "upbeat" | "down" | "anxious" | "surprised";

/** How Tally shows each tone it hears: a face, a paper colour and a few words. */
export const TONE_LOOK: Record<Tone, { mood: TallyMood; paper: TallyPaper; emoji: string; heard: string; doing: string }> = {
  calm: { mood: "hello", paper: "mint", emoji: "😌", heard: "Calm", doing: "all good" },
  frustrated: { mood: "needs", paper: "peach", emoji: "😤", heard: "Frustrated", doing: "keeping it calm" },
  upbeat: { mood: "ready", paper: "butter", emoji: "✨", heard: "Upbeat", doing: "matching your energy" },
  down: { mood: "needs", paper: "sky", emoji: "🫶", heard: "A bit down", doing: "going gently" },
  anxious: { mood: "needs", paper: "lilac", emoji: "😮‍💨", heard: "Stressed", doing: "keeping it simple" },
  surprised: { mood: "listening", paper: "sky", emoji: "😮", heard: "Surprised", doing: "noted" },
};

export function ago(seconds: number): string {
  if (seconds < 60) return "just now";
  const m = Math.round(seconds / 60);
  if (m < 60) return `${m} min ago`;
  return `${Math.round(m / 60)} h ago`;
}
