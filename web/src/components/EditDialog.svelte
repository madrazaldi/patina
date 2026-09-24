<script lang="ts">
  // Metadata editor, shown while `app.editTarget` is set. Only changed fields are sent.
  import { api, type TagEdit } from "../lib/api";
  import * as plex from "../lib/plex";
  import { act, app, message } from "../lib/state.svelte";
  import type { Metadata, Tag } from "../lib/types";

  type Field = "title" | "titleSort" | "originalTitle" | "editionTitle" | "year" | "originallyAvailableAt" | "contentRating" | "studio" | "tagline" | "summary";

  const LABELS: Record<Field, string> = {
    title: "Title",
    titleSort: "Sort title",
    originalTitle: "Original title",
    editionTitle: "Edition",
    year: "Year",
    originallyAvailableAt: "Release date",
    contentRating: "Content rating",
    studio: "Studio",
    tagline: "Tagline",
    summary: "Summary",
  };

  const FIELDS: Record<string, Field[]> = {
    movie: ["title", "titleSort", "originalTitle", "editionTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
    show: ["title", "titleSort", "originalTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
    season: ["title", "summary"],
    episode: ["title", "titleSort", "originallyAvailableAt", "summary"],
    artist: ["title", "titleSort", "summary"],
    album: ["title", "titleSort", "year", "originallyAvailableAt", "studio", "summary"],
    track: ["title", "titleSort", "originalTitle"],
    collection: ["title", "titleSort", "contentRating", "summary"],
  };
  const TAGS: Record<string, ("genre" | "label")[]> = {
    movie: ["genre", "label"],
    show: ["genre", "label"],
    artist: ["genre", "label"],
    album: ["genre", "label"],
    collection: ["label"],
  };

  let item = $derived(app.editTarget);
  let full = $state<Metadata | null>(null);
  let error = $state<string>();
  let busy = $state(false);
  let form = $state<Record<string, string>>({});
  let original: Record<string, string> = {};

  let kind = $derived(item ? plex.kind(item) : "");
  let fields = $derived(FIELDS[kind] ?? ["title", "titleSort", "summary"]);
  let tags = $derived(TAGS[kind] ?? []);
  let short = $derived(fields.filter((f) => f !== "summary"));

  const label = (f: Field) =>
    kind === "album" && f === "studio" ? "Record label" : kind === "track" && f === "originalTitle" ? "Track artist" : LABELS[f];

  const tagList = (list: Tag[]) => list.map((t) => t.tag ?? "").filter(Boolean);
  const split = (s: string) => [...new Set(s.split(",").map((t) => t.trim()).filter(Boolean))];

  function valuesOf(m: Metadata): Record<string, string> {
    const v: Record<string, string> = {};
    for (const f of Object.keys(LABELS) as Field[]) {
      const x = m[f];
      v[f] = x == null ? "" : String(x);
    }
    v.genre = tagList(m.Genre).join(", ");
    v.label = tagList(m.Label).join(", ");
    return v;
  }

  // Menu items may be partial, so edit the full item.
  $effect(() => {
    const target = item;
    full = null;
    error = undefined;
    busy = false;
    if (!target) return;
    api.metadata(target.ratingKey ?? "").then(
      (m) => {
        if (app.editTarget !== target) return;
        if (!m) {
          error = "This item no longer exists on the server.";
          return;
        }
        original = valuesOf(m);
        form = { ...original };
        full = m;
      },
      (e) => (error = message(e)),
    );
  });

  let invalid = $derived.by(() => {
    const year = form.year?.trim();
    if (fields.includes("year") && year && !/^\d{4}$/.test(year)) return "Year should be four digits.";
    const date = form.originallyAvailableAt?.trim();
    if (fields.includes("originallyAvailableAt") && date && !/^\d{4}-\d{2}-\d{2}$/.test(date)) return "Release date should be YYYY-MM-DD.";
    return null;
  });

  function changes() {
    const out: [string, string][] = fields
      .filter((f) => (form[f] ?? "").trim() !== original[f])
      .map((f) => [f, (form[f] ?? "").trim()]);
    const edits: TagEdit[] = [];
    for (const tag of tags) {
      const before = split(original[tag]);
      const after = split(form[tag] ?? "");
      const has = (list: string[], t: string) => list.some((x) => x.toLowerCase() === t.toLowerCase());
      const add = after.filter((t) => !has(before, t));
      const remove = before.filter((t) => !has(after, t));
      if (add.length || remove.length) edits.push({ tag, add, remove });
    }
    return { fields: out, tags: edits };
  }

  let dirty = $derived.by(() => {
    if (!full) return false;
    const c = changes();
    return c.fields.length > 0 || c.tags.length > 0;
  });

  function close() {
    if (!busy) app.editTarget = null;
  }

  async function save() {
    if (!full || !dirty || invalid) return;
    const c = changes();
    busy = true;
    const ok = await act(api.editMetadata(full.ratingKey ?? "", c.fields, c.tags), "Saved", true);
    busy = false;
    if (ok) app.editTarget = null;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && item && close()} />

{#if item}
  <div class="modal-backdrop" role="presentation" onclick={close}>
    <div class="modal wide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>Edit</h2>
      <p class="desc">{plex.fullTitle(full ?? item)}</p>

      {#if error}
        <div class="error-box">{error}</div>
      {:else if !full}
        <div class="spinner"></div>
      {:else}
        <form
          class="fields"
          onsubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          {#each short as f (f)}
            <label class={{ wide: f === "title" || f === "tagline" }}>
              <span>{label(f)}</span>
              {#if f === "originallyAvailableAt"}
                <input class="input" type="date" disabled={busy} bind:value={form[f]} />
              {:else}
                <input
                  class="input"
                  inputmode={f === "year" ? "numeric" : undefined}
                  placeholder={f === "titleSort" ? (form.title ?? "") : ""}
                  disabled={busy}
                  bind:value={form[f]}
                />
              {/if}
            </label>
          {/each}
          {#each tags as t (t)}
            <label>
              <span>{t === "genre" ? "Genres" : "Labels"}</span>
              <input class="input" placeholder="Comma-separated" disabled={busy} bind:value={form[t]} />
            </label>
          {/each}
          {#if fields.includes("summary")}
            <label class="wide">
              <span>Summary</span>
              <textarea class="input area" rows="6" disabled={busy} bind:value={form.summary}></textarea>
            </label>
          {/if}
          <button type="submit" hidden aria-hidden="true"></button>
        </form>
      {/if}

      <div class="foot">
        <span class={["hint", invalid ? "bad" : "faint"]}>{invalid ?? "Edited fields are locked, so refreshing metadata won't overwrite them."}</span>
        <button class="btn quiet" disabled={busy} onclick={close}>Cancel</button>
        <button class="btn primary" disabled={!dirty || !!invalid || busy} onclick={save}>{busy ? "Saving…" : "Save"}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px 14px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  label.wide {
    grid-column: 1 / -1;
  }
  label span {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .area {
    height: auto;
    padding: 8px 10px;
    line-height: 1.55;
    resize: vertical;
  }
  .foot .hint {
    flex: 1;
    align-self: center;
    font-size: 12px;
  }
  .foot .bad {
    color: var(--danger);
  }
</style>
