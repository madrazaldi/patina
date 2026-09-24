// Small formatting helpers for the UI.

import type { Metadata } from "./types";

/** "2h 14m", "48m", "35s". */
export function duration(ms: number) {
  const secs = Math.floor(ms / 1000);
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h === 0 && m === 0) return `${secs}s`;
  if (h === 0) return `${m}m`;
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

/** "1:02:03" / "4:05". */
export function clock(ms: number) {
  const secs = Math.floor(Math.max(0, ms) / 1000);
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function remaining(m: Metadata) {
  if (m.duration == null || m.viewOffset == null) return null;
  const left = m.duration - m.viewOffset;
  return left > 0 ? `${duration(left)} left` : null;
}

export function bytes(n: number) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${n} B` : `${v.toFixed(1)} ${units[i]}`;
}

export function bitrate(kbps: number) {
  return kbps >= 1000 ? `${(kbps / 1000).toFixed(1)} Mbps` : `${kbps} kbps`;
}

const dateFmt = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", year: "numeric" });

/** "Mar 4, 2024" from a unix timestamp. */
export function date(ts: number) {
  return dateFmt.format(new Date(ts * 1000));
}

/** "3 h ago", "yesterday", "Mar 4, 2024". */
export function ago(ts: number) {
  const d = Date.now() / 1000 - ts;
  if (d < 60) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  if (d < 172800) return "yesterday";
  if (d < 86400 * 7) return `${Math.floor(d / 86400)} days ago`;
  return date(ts);
}

/** "2024-03-04" → "Mar 4, 2024". */
export function airDate(s: string) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  return m ? dateFmt.format(new Date(+m[1], +m[2] - 1, +m[3])) : s;
}

export function initials(name: string) {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0])
    .join("")
    .toUpperCase();
}

export function count(n: number) {
  return n.toLocaleString("en-US");
}

export function plural(n: number, word: string, many = `${word}s`) {
  return `${count(n)} ${n === 1 ? word : many}`;
}
