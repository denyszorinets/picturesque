/** A log message split for display: what it is about, and what happened. */
export interface LogLine {
  /** The file (or thing) the message is about, or `null` if it names none. */
  subject: string | null;
  /** The rest of the message, in pieces that may wrap after each `/`. */
  detail: string[];
}

/**
 * Splits `IMG_0042.JPG: imported to 2026/2026-10-12/IMG_0042.JPG` into its
 * subject and detail. Paths in the detail break after a `/`, never inside a
 * name such as `2026-10-12`.
 */
export function logLine(message: string): LogLine {
  const at = message.indexOf(": ");
  const subject = at > 0 ? message.slice(0, at) : null;
  const rest = at > 0 ? message.slice(at + 2) : message;
  return { subject, detail: rest.split(/(?<=\/)/) };
}
