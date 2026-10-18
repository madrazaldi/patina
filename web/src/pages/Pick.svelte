<script module lang="ts">
  import type { PickQuery } from "../lib/types";

  /** The form as the user left it, kept for the session so coming back doesn't start over. */
  interface Form {
    kind: PickQuery["kind"];
    section: string;
    unwatched: boolean;
    maxMinutes: number | null;
    minRating: number | null;
    yearMin: string;
    yearMax: string;
    genres: string[];
    contentRatings: string[];
  }
  const fresh = (): Form => ({ kind: "movie", section: "", unwatched: true, maxMinutes: null, minRating: null, yearMin: "", yearMax: "", genres: [], contentRatings: [] });
  let remembered: { scope: string; form: Form } | null = null;
</script>

<script lang="ts">
  // "Pick something to watch": a few random candidates that fit what you feel like.
  import { Dices } from "@lucide/svelte";
  import { untrack } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import * as Field from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { Kbd } from "$lib/components/ui/kbd";
  import { Label } from "$lib/components/ui/label";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import Card from "../components/Card.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { resource } from "../lib/resource.svelte";
  import { app, captureScope, message } from "../lib/state.svelte";
  import type { PickOptions, PickResult } from "../lib/types";

  const guard = captureScope();
  const options = resource(() => api.pickOptions());

  let form = $state<Form>(remembered && remembered.scope === app.scope ? structuredClone($state.snapshot(remembered.form)) : fresh());
  $effect(() => {
    remembered = { scope: app.scope, form: $state.snapshot(form) as Form };
  });

  const RUNTIMES = [30, 60, 90, 120, 150, 180];
  const RATINGS = [5, 6, 7, 8];

  let sections = $derived((options.data?.sections ?? []).filter((s) => form.kind === "any" || s.kind === form.kind));
  // A library of the other kind no longer applies once the kind changes.
  $effect(() => {
    if (form.section && !sections.some((s) => s.id === form.section)) form.section = "";
  });

  const year = (v: string) => {
    const n = Number.parseInt(v, 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  };
  let query = $derived<PickQuery>({
    kind: form.kind,
    unwatched: form.unwatched,
    maxMinutes: form.maxMinutes,
    minMinutes: null,
    genres: form.genres,
    contentRatings: form.contentRatings,
    sections: form.section ? [form.section] : [],
    yearMin: year(form.yearMin),
    yearMax: year(form.yearMax),
    minRating: form.minRating,
    count: 4,
    exclude: [],
  });

  let result = $state<PickResult | null>(null);
  let error = $state<string | undefined>();
  let busy = $state(false);
  /** Rating keys already offered for these criteria, so a reroll shows something new. */
  let seen: string[] = [];
  let token = 0;

  async function roll(again: boolean) {
    const mine = ++token;
    const q = $state.snapshot(query) as PickQuery;
    busy = true;
    error = undefined;
    try {
      const r = await api.pick({ ...q, exclude: again ? seen : [] });
      if (mine !== token || !guard.valid()) return;
      result = r;
      const keys = r.items.map((m) => m.ratingKey ?? "");
      seen = again && !r.wrapped ? [...seen, ...keys] : keys;
    } catch (e) {
      if (mine === token && guard.valid()) error = message(e);
    } finally {
      if (mine === token) busy = false;
    }
  }

  // New criteria start a fresh draw, shortly after the last change.
  $effect(() => {
    JSON.stringify(query);
    const timer = setTimeout(() => untrack(() => roll(false)), 250);
    return () => clearTimeout(timer);
  });

  const toggle = (list: string[], value: string) => (list.includes(value) ? list.filter((v) => v !== value) : [...list, value]);

  function onKey(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey || e.key.toLowerCase() !== "r" || document.querySelector("dialog[open]")) return;
    const t = e.target as HTMLElement | null;
    if (t && (["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName) || t.isContentEditable)) return;
    e.preventDefault();
    void roll(true);
  }

  let choices = $derived<PickOptions>(options.data ?? { sections: [], genres: [], contentRatings: [] });
</script>

<svelte:window onkeydown={onKey} />

<div class="page">
  <div class="page-head">
    <div>
      <h1>Pick something to watch</h1>
      <div class="sub">A few random candidates that fit what you feel like. Press R to reroll.</div>
    </div>
  </div>

  <div class="grid items-start gap-9 min-[861px]:grid-cols-[280px_minmax(0,1fr)]">
    <form class="flex flex-col gap-4 rounded-xl border bg-card p-4" onsubmit={(e) => e.preventDefault()} aria-label="What to watch">
      <Field.Field>
        <Field.Label for="pick-kind">Looking for</Field.Label>
        <NativeSelect.Root id="pick-kind" class="w-full" bind:value={form.kind}>
          <NativeSelect.Option value="movie">Movies</NativeSelect.Option>
          <NativeSelect.Option value="show">Shows</NativeSelect.Option>
          <NativeSelect.Option value="any">Movies and shows</NativeSelect.Option>
        </NativeSelect.Root>
      </Field.Field>
      <Field.Field>
        <Field.Label for="pick-section">Library</Field.Label>
        <NativeSelect.Root id="pick-section" class="w-full" bind:value={form.section}>
          <NativeSelect.Option value="">Any library</NativeSelect.Option>
          {#each sections as s (s.id)}<NativeSelect.Option value={s.id}>{s.title}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </Field.Field>
      <Field.Field orientation="horizontal">
        <Checkbox id="pick-unwatched" bind:checked={form.unwatched} />
        <Label for="pick-unwatched" class="cursor-pointer">Only unwatched</Label>
      </Field.Field>
      <Field.Field>
        <Field.Label for="pick-runtime">Runtime</Field.Label>
        <NativeSelect.Root id="pick-runtime" class="w-full" value={form.maxMinutes ?? ""} onchange={(e) => (form.maxMinutes = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
          <NativeSelect.Option value="">Any length</NativeSelect.Option>
          {#each RUNTIMES as m (m)}<NativeSelect.Option value={m}>Up to {m} minutes</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </Field.Field>
      <Field.Field>
        <Field.Label for="pick-rating">Audience rating</Field.Label>
        <NativeSelect.Root id="pick-rating" class="w-full" value={form.minRating ?? ""} onchange={(e) => (form.minRating = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
          <NativeSelect.Option value="">Any rating</NativeSelect.Option>
          {#each RATINGS as r (r)}<NativeSelect.Option value={r}>{r}.0 or higher</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </Field.Field>
      <div class="grid grid-cols-2 gap-2.5">
        <Field.Field>
          <Field.Label for="pick-year-min">From year</Field.Label>
          <Input id="pick-year-min" inputmode="numeric" placeholder="Any" bind:value={form.yearMin} />
        </Field.Field>
        <Field.Field>
          <Field.Label for="pick-year-max">To year</Field.Label>
          <Input id="pick-year-max" inputmode="numeric" placeholder="Any" bind:value={form.yearMax} />
        </Field.Field>
      </div>

      {#if choices.genres.length}
        <Field.Set class="min-w-0 gap-1">
          <Field.Legend variant="label" class="mb-1.5">Genres <span class="font-normal text-muted-foreground">any of</span></Field.Legend>
          <div class="flex max-h-52 flex-wrap gap-1.5 overflow-y-auto">
            {#each choices.genres as g (g)}
              {@const on = form.genres.includes(g)}
              <Button variant={on ? "default" : "outline"} size="sm" class={["rounded-full", { active: on }]} aria-pressed={on} onclick={() => (form.genres = toggle(form.genres, g))}>{g}</Button>
            {/each}
          </div>
        </Field.Set>
      {/if}
      {#if choices.contentRatings.length}
        <Field.Set class="min-w-0 gap-1">
          <Field.Legend variant="label" class="mb-1.5">Content rating <span class="font-normal text-muted-foreground">any of</span></Field.Legend>
          <div class="flex max-h-52 flex-wrap gap-1.5 overflow-y-auto">
            {#each choices.contentRatings as c (c)}
              {@const on = form.contentRatings.includes(c)}
              <Button variant={on ? "default" : "outline"} size="sm" class={["rounded-full", { active: on }]} aria-pressed={on} onclick={() => (form.contentRatings = toggle(form.contentRatings, c))}>{c}</Button>
            {/each}
          </div>
        </Field.Set>
      {/if}
      <Button variant="ghost" size="sm" class="-ml-2.5 self-start" onclick={() => (form = fresh())}>Reset</Button>
    </form>

    <section class="results min-w-0" aria-live="polite" aria-busy={busy}>
      {#if error}
        <Status {error} retry={() => roll(false)} />
      {:else if !result}
        <Status loading />
      {:else if !result.items.length}
        <Status title="Nothing fits" text="Loosen a filter, such as the runtime, rating or years." />
      {:else}
        <div class="mb-5 flex items-center gap-3.5 text-sm">
          <Button disabled={busy} onclick={() => roll(true)}><Dices />Reroll <Kbd class="bg-primary-foreground/15 text-primary-foreground">R</Kbd></Button>
          <span class="text-muted-foreground">
            {#if result.wrapped}
              That was everything that fits ({result.matches}); starting over.
            {:else}
              {result.matches} {result.matches === 1 ? "title fits" : "titles fit"}{result.remaining ? `, ${result.remaining} not offered yet` : ""}
            {/if}
            {#if result.libraries.length} · {result.libraries.join(", ")}{/if}
          </span>
        </div>
        <div class="poster-grid">
          {#each result.items as item (item.ratingKey)}
            <Card {item} />
          {/each}
        </div>
      {/if}
    </section>
  </div>
</div>
