// Lenient decoding helpers (port of crates/plex/src/de.rs). Plex is inconsistent about JSON
// types: ids come as strings or numbers, booleans as `true`, `1` or `"1"`, and so on.
// Models are built by explicit normalizers that copy only whitelisted fields; the helpers
// here do the scalar coercions and the single-object-where-a-list-is-expected handling.

/** A decoding problem, with the serde_path_to_error style path of the offending value. */
export class DecodeFailure extends Error {
  /** Dotted path such as `MediaContainer.Metadata[0].Media`; empty for the root. */
  readonly path: string;
  /** The message without the path. */
  readonly detail: string;

  constructor(path: string, detail: string) {
    super(path ? `${path}: ${detail}` : detail);
    this.name = "DecodeFailure";
    this.path = path;
    this.detail = detail;
  }

  /** The same failure seen from inside `segment` (a field name or `name[index]`). */
  under(segment: string): DecodeFailure {
    const path = this.path === "" ? segment : this.path.startsWith("[") ? segment + this.path : `${segment}.${this.path}`;
    return new DecodeFailure(path, this.detail);
  }
}

/** serde's wording for "what we found" in `invalid type` errors. */
function describe(v: unknown): string {
  if (v === null || v === undefined) return "null";
  if (typeof v === "boolean") return `boolean \`${v}\``;
  if (typeof v === "number") return Number.isInteger(v) ? `integer \`${v}\`` : `floating point \`${v}\``;
  if (typeof v === "string") return `string ${JSON.stringify(v)}`;
  if (Array.isArray(v)) return "sequence";
  return "map";
}

/** Asserts `v` is a JSON object (what a serde struct needs) and returns it. */
export function obj(v: unknown, expected: string): Record<string, unknown> {
  if (typeof v !== "object" || v === null || Array.isArray(v)) {
    throw new DecodeFailure("", `invalid type: ${describe(v)}, expected struct ${expected}`);
  }
  return v as Record<string, unknown>;
}

/** Scalars as the Rust `AnyScalar` visitor sees them. Objects and arrays count as absent. */
type Scalar = { t: "bool"; v: boolean } | { t: "num"; v: number } | { t: "str"; v: string } | { t: "null" };

function scalar(v: unknown): Scalar {
  if (typeof v === "boolean") return { t: "bool", v };
  if (typeof v === "number") return { t: "num", v };
  if (typeof v === "string") return { t: "str", v };
  return { t: "null" };
}

const I64_MAX = 9223372036854775807;

/** Rust's `f64 as i64`: truncates, saturates and maps NaN to 0. */
function toI64(f: number): number {
  if (Number.isNaN(f)) return 0;
  if (f >= I64_MAX) return I64_MAX;
  if (f <= -I64_MAX) return -I64_MAX;
  return Math.trunc(f);
}

/** Rust's `str::parse::<f64>()` (no surrounding whitespace, accepts inf/nan). */
function parseF64(s: string): number | null {
  const t = s;
  if (/^[+-]?(\d+\.?\d*([eE][+-]?\d+)?|\.\d+([eE][+-]?\d+)?)$/.test(t)) return Number(t);
  const lower = t.toLowerCase();
  if (/^[+-]?(inf|infinity)$/.test(lower)) return lower.startsWith("-") ? -Infinity : Infinity;
  if (/^[+-]?nan$/.test(lower)) return NaN;
  return null;
}

export function optString(v: unknown): string | null {
  const s = scalar(v);
  switch (s.t) {
    case "str":
      return s.v;
    case "num":
      return String(s.v);
    case "bool":
      return String(s.v);
    case "null":
      return null;
  }
}

export function optI64(v: unknown): number | null {
  const s = scalar(v);
  switch (s.t) {
    case "num":
      return toI64(s.v);
    case "str": {
      const f = parseF64(s.v.trim());
      return f === null ? null : toI64(f);
    }
    case "bool":
      return s.v ? 1 : 0;
    case "null":
      return null;
  }
}

export function optF64(v: unknown): number | null {
  const s = scalar(v);
  switch (s.t) {
    case "num":
      return s.v;
    case "str":
      return parseF64(s.v.trim());
    case "bool":
      return s.v ? 1 : 0;
    case "null":
      return null;
  }
}

export function boolean(v: unknown): boolean {
  const s = scalar(v);
  switch (s.t) {
    case "bool":
      return s.v;
    case "num":
      return s.v !== 0;
    case "str":
      return s.v === "1" || s.v === "true" || s.v === "True" || s.v === "yes";
    case "null":
      return false;
  }
}

/** A list that Plex sometimes sends as a single object (e.g. `OnDeck.Metadata`). */
export function oneOrMany<T>(v: unknown, f: (item: unknown) => T, key = ""): T[] {
  if (v === null || v === undefined) return [];
  if (Array.isArray(v)) return mapItems(v, f, key);
  return [underKey(key, () => f(v))];
}

/** A plain `Vec<T>` field: absent means empty. */
export function list<T>(v: unknown, f: (item: unknown) => T, key = ""): T[] {
  if (v === null || v === undefined) return [];
  if (!Array.isArray(v)) {
    throw new DecodeFailure(key, `invalid type: ${describe(v)}, expected a sequence`);
  }
  return mapItems(v, f, key);
}

/** An `Option<T>` struct field: absent or null means `null`. */
export function optional<T>(v: unknown, f: (item: unknown) => T, key = ""): T | null {
  if (v === null || v === undefined) return null;
  return underKey(key, () => f(v));
}

/** A `serde_json::Value` field: absent means null. */
export function value(v: unknown): unknown {
  return v === undefined ? null : v;
}

/** The attributes of `o` that are not in `known` (serde's `#[serde(flatten)]` catch-all map). */
export function extraOf(o: Record<string, unknown>, known: ReadonlySet<string>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(o)) {
    if (!known.has(k)) out[k] = o[k];
  }
  return out;
}

function mapItems<T>(items: unknown[], f: (item: unknown) => T, key: string): T[] {
  const out: T[] = [];
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    out.push(underKey(`${key}[${i}]`, () => f(item)));
  }
  return out;
}

function underKey<T>(segment: string, f: () => T): T {
  try {
    return f();
  } catch (e) {
    if (e instanceof DecodeFailure && segment) throw e.under(segment);
    throw e;
  }
}

/** Navigate a JSON value like `Value::pointer` (`/A/B`). */
export function pointer(v: unknown, path: string): unknown {
  let cur: unknown = v;
  for (const seg of path.split("/").slice(1)) {
    const key = seg.replace(/~1/g, "/").replace(/~0/g, "~");
    if (Array.isArray(cur)) {
      if (!/^(0|[1-9]\d*)$/.test(key)) return undefined;
      cur = cur[Number(key)];
    } else if (typeof cur === "object" && cur !== null) {
      if (!Object.hasOwn(cur, key)) return undefined;
      cur = (cur as Record<string, unknown>)[key];
    } else {
      return undefined;
    }
    if (cur === undefined) return undefined;
  }
  return cur;
}
