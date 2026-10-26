import { MediaType, type Metadata, normalizeMetadata } from "../../plex";

// Small helpers shared by the ops modules (the Rust code used futures streams
// and anyhow here).

/**
 * Run `fn` over `items` with at most `limit` in flight and collect the results
 * in input order (`stream::iter(..).map(fn).buffered(limit).try_collect()`).
 * The first failure rejects the whole call; work already started is not
 * cancelled but nothing new is started.
 */
export async function mapLimit<T, R>(items: readonly T[], limit: number, fn: (item: T, index: number) => Promise<R>): Promise<R[]> {
  const out: R[] = new Array(items.length);
  let next = 0;
  let failed = false;
  const worker = async () => {
    while (!failed) {
      const at = next++;
      if (at >= items.length) return;
      try {
        out[at] = await fn(items[at]!, at);
      } catch (e) {
        failed = true;
        throw e;
      }
    }
  };
  const workers = Array.from({ length: Math.min(limit, items.length) }, worker);
  await Promise.all(workers);
  return out;
}

/**
 * Run `fn` over `items` with at most `limit` in flight and hand each result to
 * `onResult` in completion order (`buffer_unordered(limit)` consumed in a loop).
 * `onResult` runs one at a time.
 */
export async function eachCompleted<T, R>(
  items: readonly T[],
  limit: number,
  fn: (item: T) => Promise<R>,
  onResult: (result: R, item: T) => void,
): Promise<void> {
  let next = 0;
  const worker = async () => {
    while (true) {
      const at = next++;
      if (at >= items.length) return;
      const item = items[at]!;
      const result = await fn(item);
      onResult(result, item);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
}

/** `format!("{e:#}")` for a caught value. */
export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** `MediaType::from_name(name).map(MediaType::id)`. */
export function mediaTypeId(name: string): number | undefined {
  return MediaType.fromName(name) ?? undefined;
}

/** `serde_json::from_value::<Metadata>`: the lenient Plex normalizer. */
export function metadataFromJson(v: unknown): Metadata {
  return normalizeMetadata(v);
}
