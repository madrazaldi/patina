// Small fuzzy matcher for the command palette: every character of the query must appear
// in the text in order. Runs, word starts and early matches score higher.

/** A score (higher is better), or null when `query` doesn't match `text`. */
export function fuzzy(query: string, text: string): number | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (!q) return 0;
  const t = text.toLowerCase();
  let score = 0;
  let from = 0;
  let previous = -2;
  for (const ch of q) {
    const at = t.indexOf(ch, from);
    if (at < 0) return null;
    score += 1;
    if (at === previous + 1) score += 4;
    if (at === 0 || /[\s\-_/:.,]/.test(t[at - 1])) score += 6;
    score -= Math.min(at - from, 6) * 0.3;
    previous = at;
    from = at + 1;
  }
  if (t.startsWith(q)) score += 12;
  else if (t.includes(q)) score += 6;
  return score - t.length * 0.02;
}

/**
 * Match a possibly multi-word query: each word has to match the label fuzzily, or failing that the start of
 * one of the keywords (which counts for less). The scores add up.
 */
export function match(query: string, label: string, keywords = ""): number | null {
  const words = query.trim().split(/\s+/).filter(Boolean);
  if (!words.length) return 0;
  const extra = keywords.toLowerCase().split(/\s+/).filter(Boolean);
  let total = 0;
  for (const word of words) {
    const direct = fuzzy(word, label);
    if (direct !== null) { total += direct; continue; }
    const lower = word.toLowerCase();
    if (!extra.some((k) => k.startsWith(lower))) return null;
    total += 2;
  }
  return total;
}
