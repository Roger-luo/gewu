/**
 * A log line as a reviewer reads it: the harness's prompt headings
 * ("# Gewu harness: waiting for the reviewer") become their plain words.
 * Found by the final UI judge.
 */
export function readableLogText(text: string): string {
  return text
    .replace(/#+\s*Gewu harness:\s*/g, "")
    .replace(/#\s*COMPLETE\b/g, "complete")
    .replace(/^#+\s+/, "");
}
