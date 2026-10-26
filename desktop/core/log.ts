// Logging for the desktop backend. Everything goes to stderr; PATINA_LOG
// ("debug", "info", "warn", "error", or "off") sets the level (default: info).

const levels = { debug: 10, info: 20, warn: 30, error: 40, off: 99 } as const;
type Level = keyof typeof levels;

function threshold(): number {
  const raw = (process.env.PATINA_LOG ?? "").toLowerCase();
  // Accepts tracing-style filters like "warn,patina=info,mpv=warn": the first bare level wins.
  const first = raw.split(",").map((s) => s.trim()).find((s) => s in levels);
  return levels[(first as Level | undefined) ?? "info"];
}

const min = threshold();

function write(level: Exclude<Level, "off">, scope: string, args: unknown[]) {
  if (levels[level] < min) return;
  const text = args.map((a) => (typeof a === "string" ? a : a instanceof Error ? a.message : JSON.stringify(a))).join(" ");
  process.stderr.write(`${new Date().toISOString()} ${level.toUpperCase().padEnd(5)} ${scope}: ${text}\n`);
}

/** A logger tagged with a module name, like `tracing`'s target. */
export function logger(scope: string) {
  return {
    debug: (...args: unknown[]) => write("debug", scope, args),
    info: (...args: unknown[]) => write("info", scope, args),
    warn: (...args: unknown[]) => write("warn", scope, args),
    error: (...args: unknown[]) => write("error", scope, args),
  };
}

export type Logger = ReturnType<typeof logger>;
