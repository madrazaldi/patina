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

  <div class="layout">
    <form class="form" onsubmit={(e) => e.preventDefault()} aria-label="What to watch">
      <label class="field">
        <span>Looking for</span>
        <select class="select" bind:value={form.kind}>
          <option value="movie">Movies</option>
          <option value="show">Shows</option>
          <option value="any">Movies and shows</option>
        </select>
      </label>
      <label class="field">
        <span>Library</span>
        <select class="select" bind:value={form.section}>
          <option value="">Any library</option>
          {#each sections as s (s.id)}<option value={s.id}>{s.title}</option>{/each}
        </select>
      </label>
      <label class="check"><input type="checkbox" bind:checked={form.unwatched} /> Only unwatched</label>
      <label class="field">
        <span>Runtime</span>
        <select class="select" value={form.maxMinutes ?? ""} onchange={(e) => (form.maxMinutes = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
          <option value="">Any length</option>
          {#each RUNTIMES as m (m)}<option value={m}>Up to {m} minutes</option>{/each}
        </select>
      </label>
      <label class="field">
        <span>Audience rating</span>
        <select class="select" value={form.minRating ?? ""} onchange={(e) => (form.minRating = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
          <option value="">Any rating</option>
          {#each RATINGS as r (r)}<option value={r}>{r}.0 or higher</option>{/each}
        </select>
      </label>
      <div class="years">
        <label class="field">
          <span>From year</span>
          <input class="input" inputmode="numeric" placeholder="Any" bind:value={form.yearMin} />
        </label>
        <label class="field">
          <span>To year</span>
          <input class="input" inputmode="numeric" placeholder="Any" bind:value={form.yearMax} />
        </label>
      </div>

      {#if choices.genres.length}
        <fieldset class="chips">
          <legend>Genres <span class="faint">any of</span></legend>
          <div class="wrap">
            {#each choices.genres as g (g)}
              {@const on = form.genres.includes(g)}
              <button type="button" class={["chip", { active: on }]} aria-pressed={on} onclick={() => (form.genres = toggle(form.genres, g))}>{g}</button>
            {/each}
          </div>
        </fieldset>
      {/if}
      {#if choices.contentRatings.length}
        <fieldset class="chips">
          <legend>Content rating <span class="faint">any of</span></legend>
          <div class="wrap">
            {#each choices.contentRatings as c (c)}
              {@const on = form.contentRatings.includes(c)}
              <button type="button" class={["chip", { active: on }]} aria-pressed={on} onclick={() => (form.contentRatings = toggle(form.contentRatings, c))}>{c}</button>
            {/each}
          </div>
        </fieldset>
      {/if}
      <button type="button" class="btn quiet sm" onclick={() => (form = fresh())}>Reset</button>
    </form>

    <section class="results" aria-live="polite" aria-busy={busy}>
      {#if error}
        <Status {error} retry={() => roll(false)} />
      {:else if !result}
        <Status loading />
      {:else if !result.items.length}
        <Status title="Nothing fits" text="Loosen a filter, such as the runtime, rating or years." />
      {:else}
        <div class="bar-row">
          <button class="btn primary" disabled={busy} onclick={() => roll(true)}><Dices size={14} />Reroll <kbd>R</kbd></button>
          <span class="faint">
            {#if result.wrapped}
              That was everything that fits ({result.matches}); starting over.
            {:else}
              {result.matches} {result.matches === 1 ? "title fits" : "titles fit"}{result.remaining ? `, ${result.remaining} not offered yet` : ""}
            {/if}
            {#if result.libraries.length} · {result.libraries.join(", ")}{/if}
          </span>
        </div>
        <div class="grid">
          {#each result.items as item (item.ratingKey)}
            <Card {item} />
          {/each}
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: 280px minmax(0, 1fr);
    gap: 36px;
    align-items: start;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    color: var(--text-2);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .years {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .chips {
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .chips legend {
    padding: 0;
    margin-bottom: 6px;
    font-size: 12px;
    color: var(--text-2);
  }
  .wrap {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    max-height: 200px;
    overflow-y: auto;
  }
  .bar-row {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 18px;
    font-size: 12.5px;
  }
  kbd {
    margin-left: 4px;
    font: 11px var(--mono);
    opacity: 0.7;
  }
  @media (max-width: 860px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
</style>
