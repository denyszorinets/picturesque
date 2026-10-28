import type { BackgroundCritiqueStatus } from "../api";
import { formatTimeLeft } from "../ai/duration";
import type { Ring } from "./ring";

/** Photos still to critique: those without one or with an outdated one. */
export function critiqueLeft(s: BackgroundCritiqueStatus): number {
  return s.missing + s.stale;
}

/** The first line: what it is doing now (or why it waits). */
export function critiqueHeadline(s: BackgroundCritiqueStatus): string {
  const left = critiqueLeft(s);
  if (s.paused && left > 0) return `Critique paused: ${s.paused}`;
  if (left > 0)
    return `${s.running ? "Critiquing" : "Critique waiting"}: ${s.critiqued} / ${s.total} done`;
  return `Critique: all ${s.total} photos critiqued`;
}

/** Redo count, speed and time left; not the headline or failures. */
export function critiqueExtraLines(s: BackgroundCritiqueStatus): string[] {
  const left = critiqueLeft(s);
  const lines: string[] = [];
  if (s.stale > 0) lines.push(`${s.stale} to redo (earlier model or prompt)`);
  if (left > 0 && s.seconds_per_photo !== null)
    lines.push(`${Math.round(s.seconds_per_photo)} s per photo`);
  if (left > 0 && s.seconds_left !== null)
    lines.push(`${formatTimeLeft(s.seconds_left)} left`);
  return lines;
}

/** The failure line, or `null` when nothing failed. */
export function critiqueFailedLine(s: BackgroundCritiqueStatus): string | null {
  return s.failed > 0 ? `${s.failed} failed (skipped until restart)` : null;
}

/** All lines, for the pill's title and popover. */
export function critiqueDetails(s: BackgroundCritiqueStatus): string[] {
  const failed = critiqueFailedLine(s);
  return [
    critiqueHeadline(s),
    ...critiqueExtraLines(s),
    ...(failed ? [failed] : []),
  ];
}

/**
 * The header's second pill. N = `critiqued` (current model and rubric),
 * M = `total` (= critiqued + stale + missing, rejects excluded). `null`
 * when the background critic is off, or idle with nothing to do.
 */
export function critiqueRing(s: BackgroundCritiqueStatus | null): Ring | null {
  if (!s || !s.enabled) return null;
  const left = critiqueLeft(s);
  if (left === 0 && !s.running && !s.paused) return null;
  const progress = s.total > 0 ? s.critiqued / s.total : 1;
  const count = `${s.critiqued} / ${s.total}`;
  const paused = !!s.paused && !s.running;
  return {
    label: paused ? `Critique paused · ${count}` : `Critique ${count}`,
    tone: paused ? "paused" : "busy",
    progress,
    failed: s.failed,
    details: critiqueDetails(s),
  };
}
