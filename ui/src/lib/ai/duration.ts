/** "about 40 min", "about 5 h" or "about 4 days" for a time left. */
export function formatTimeLeft(seconds: number): string {
  const minutes = Math.max(1, Math.round(seconds / 60));
  if (seconds < 2 * 3600) return `about ${minutes} min`;
  if (seconds < 3 * 86400) return `about ${Math.round(seconds / 3600)} h`;
  return `about ${Math.round(seconds / 86400)} days`;
}
