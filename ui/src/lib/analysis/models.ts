/**
 * Tells what is not analyzed until its AI model is downloaded
 * (`waiting_for_models` of the analysis status), or `null` when nothing
 * waits.
 */
export function waitingNote(waiting: readonly string[]): string | null {
  if (waiting.length === 0) return null;
  return `AI analysis needs models (${waiting.join("; ")}): download them in Settings → AI models.`;
}
