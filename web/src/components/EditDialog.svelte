<script lang="ts">
  // Metadata editor, shown while `app.editTarget` is set. Only changed fields are sent.
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { Field, FieldGroup, FieldLabel } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Textarea } from "$lib/components/ui/textarea";
  import { modal } from "../lib/dialog";
  import { api, type TagEdit } from "../lib/api";
  import * as plex from "../lib/plex";
  import { act, app, captureScope, message } from "../lib/state.svelte";
  import type { Metadata, Tag } from "../lib/types";

  type FieldKey = "title" | "titleSort" | "originalTitle" | "editionTitle" | "year" | "originallyAvailableAt" | "contentRating" | "studio" | "tagline" | "summary";

  const LABELS: Record<FieldKey, string> = {
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

  const FIELDS: Record<string, FieldKey[]> = {
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

  const label = (f: FieldKey) =>
    kind === "album" && f === "studio" ? "Record label" : kind === "track" && f === "originalTitle" ? "Track artist" : LABELS[f];

  const tagList = (list: Tag[]) => list.map((t) => t.tag ?? "").filter(Boolean);
  const split = (s: string) => [...new Set(s.split(",").map((t) => t.trim()).filter(Boolean))];

  function valuesOf(m: Metadata): Record<string, string> {
    const v: Record<string, string> = {};
    for (const f of Object.keys(LABELS) as FieldKey[]) {
      const x = m[f];
      v[f] = x == null ? "" : String(x);
    }
    v.genre = tagList(m.Genre).join(", ");
    v.label = tagList(m.Label).join(", ");
    return v;
  }

  // The account and server the editor was opened on. A save or a late response from any other one is dropped.
  let guard = captureScope();

  // Menu items may be partial, so edit the full item.
  $effect(() => {
    const target = item;
    full = null;
    error = undefined;
    busy = false;
    if (!target) return;
    const opened = (guard = captureScope());
    api.metadata(target.ratingKey ?? "").then(
      (m) => {
        if (app.editTarget !== target || !opened.valid()) return;
        if (!m) {
          error = "This item no longer exists on the server.";
          return;
        }
        original = valuesOf(m);
        form = { ...original };
        full = m;
      },
      (e) => app.editTarget === target && opened.valid() && (error = message(e)),
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
    // The editor holds a rating key from the server it was opened on; never send it to another one.
    if (!guard.valid()) { app.editTarget = null; return; }
    const c = changes();
    busy = true;
    const ok = await act(api.editMetadata(full.ratingKey ?? "", c.fields, c.tags), "Saved", true);
    busy = false;
    if (ok && guard.valid()) app.editTarget = null;
  }
</script>

{#if item}
  <dialog class="modal wide" use:modal={close} aria-labelledby="edit-title">
    <h2 id="edit-title">Edit</h2>
    <p class="desc">{plex.fullTitle(full ?? item)}</p>

    {#if error}
      <Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap select-text">{error}</Alert.Description></Alert.Root>
    {:else if !full}
      <div class="flex justify-center py-6"><Spinner class="text-muted-foreground" /></div>
    {:else}
      <form
        onsubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <FieldGroup class="grid grid-cols-2 gap-x-3.5 gap-y-4">
          {#each short as f (f)}
            <Field class={f === "title" || f === "tagline" ? "col-span-2" : "min-w-0"}>
              <FieldLabel for="edit-field-{f}">{label(f)}</FieldLabel>
              {#if f === "originallyAvailableAt"}
                <Input id="edit-field-{f}" type="date" disabled={busy} bind:value={form[f]} />
              {:else}
                <Input
                  id="edit-field-{f}"
                  data-initial-focus={f === "title" ? "" : undefined}
                  inputmode={f === "year" ? "numeric" : undefined}
                  placeholder={f === "titleSort" ? (form.title ?? "") : ""}
                  disabled={busy}
                  bind:value={form[f]}
                />
              {/if}
            </Field>
          {/each}
          {#each tags as t (t)}
            <Field class="min-w-0">
              <FieldLabel for="edit-field-{t}">{t === "genre" ? "Genres" : "Labels"}</FieldLabel>
              <Input id="edit-field-{t}" placeholder="Comma-separated" disabled={busy} bind:value={form[t]} />
            </Field>
          {/each}
          {#if fields.includes("summary")}
            <Field class="col-span-2">
              <FieldLabel for="edit-field-summary">Summary</FieldLabel>
              <Textarea id="edit-field-summary" rows={6} class="resize-y leading-relaxed" disabled={busy} bind:value={form.summary} />
            </Field>
          {/if}
        </FieldGroup>
        <button type="submit" hidden aria-hidden="true"></button>
      </form>
    {/if}

    <div class="foot items-center">
      <span class={["mr-auto text-xs", invalid ? "text-destructive" : "text-muted-foreground"]}>{invalid ?? "Edited fields are locked, so refreshing metadata won't overwrite them."}</span>
      <Button variant="outline" disabled={busy} onclick={close}>Cancel</Button>
      <Button disabled={!dirty || !!invalid || busy} onclick={save}>
        {#if busy}<Spinner />{/if}
        {busy ? "Saving…" : "Save"}
      </Button>
    </div>
  </dialog>
{/if}
