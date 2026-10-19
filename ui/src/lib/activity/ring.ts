import type { AnalysisStatus, PipelineStatus } from "../api";

/** What the header's progress ring shows. */
export interface Ring {
  /** Short text next to the ring. */
  label: string;
  tone: "busy" | "paused" | "done";
  /** How full the ring is, 0–1, or `null` while the amount is unknown. */
  progress: number | null;
  /** Photos that failed to import, shown as a red count. */
  failed: number;
  /** One line per kind of work, for the details popover. */
  details: string[];
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

function importLine(p: PipelineStatus | null): string | null {
  if (!p) return null;
  if (p.paused && p.queued > 0)
    return `Import paused: ${p.paused} (${plural(p.queued, "photo")} waiting)`;
  if (p.queued > 0) {
    const doing = p.active
      ? ` — ${p.active.stage === "reading" ? "reading" : "copying"} ${p.active.file_name}`
      : "";
    return `Importing: ${p.queued} left${doing}`;
  }
  if (p.scanning) return "Looking for new photos in the import folders";
  return `Import: ${plural(p.imported, "photo")} imported, nothing waiting`;
}

function analysisLine(a: AnalysisStatus | null): string | null {
  if (!a) return null;
  const remaining = a.total - a.analyzed;
  if (a.paused && remaining > 0)
    return `Analysis paused: ${a.paused.split(";")[0]} (${a.analyzed} / ${a.total})`;
  if (remaining > 0)
    return `Analyzing: ${a.analyzed} / ${a.total} (photos on screen go first)`;
  return `Analysis: all ${plural(a.total, "photo")} analyzed`;
}

/**
 * Combines import and analysis progress into one ring: importing first,
 * then analysis, then done. `null` until either status is known.
 */
export function ringState(
  pipeline: PipelineStatus | null,
  analysis: AnalysisStatus | null,
): Ring | null {
  if (!pipeline && !analysis) return null;
  const failed = pipeline?.failed ?? 0;
  const details = [importLine(pipeline), analysisLine(analysis)].filter(
    (line): line is string => line !== null,
  );
  if (failed > 0)
    details.push(`${plural(failed, "photo")} could not be imported`);
  const ring = (
    tone: Ring["tone"],
    label: string,
    progress: number | null,
  ) => ({
    label,
    tone,
    progress,
    failed,
    details,
  });

  if (pipeline && pipeline.queued > 0) {
    return pipeline.paused
      ? ring("paused", "Import paused", null)
      : ring("busy", `Importing · ${pipeline.queued} left`, null);
  }
  if (pipeline?.scanning) return ring("busy", "Looking for photos", null);
  if (analysis && analysis.analyzed < analysis.total) {
    const progress = analysis.analyzed / analysis.total;
    return analysis.paused
      ? ring("paused", "Analysis paused", progress)
      : ring(
          "busy",
          `Analyzing ${analysis.analyzed} / ${analysis.total}`,
          progress,
        );
  }
  return ring("done", "Up to date", 1);
}
