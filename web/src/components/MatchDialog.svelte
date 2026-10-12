<script lang="ts">
  // "Fix match" dialog, shown while `app.matchTarget` is set.
  import { untrack } from "svelte";
  import Img from "./Img.svelte";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { act, app, captureScope, message } from "../lib/state.svelte";
  import type { MatchResult } from "../lib/types";

  let item = $derived(app.matchTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let title = $state("");
  let year = $state("");
  let results = $state<MatchResult[] | null>(null);
  let error = $state<string>();
  let busy = $state(false);
  let generation = 0;

  let square = $derived(!!item && ["artist", "album"].includes(plex.kind(item)));

  function search(manual: boolean) {
    const target = item;
    if (!target) return;
    const run = ++generation;
    const asked = guard;
    results = null;
    error = undefined;
    const y = Number(year.trim());
    const q = manual ? title.trim() || undefined : undefined;
    api.matches(target.ratingKey ?? "", q, manual && y ? y : undefined).then(
      (r) => run === generation && asked.valid() && (results = r),
      (e) => run === generation && asked.valid() && (error = message(e)),
    );
  }

  // Automatic matches on open.
  $effect(() => {
    const target = item;
    busy = false;
    if (!target) return;
    guard = captureScope();
    title = plex.title(target);
    year = target.year != null ? String(target.year) : "";
    untrack(() => search(false));
  });

  function close() {
    if (!busy) app.matchTarget = null;
  }

  async function apply(r: MatchResult) {
    if (!item || !r.guid || busy) return;
    if (!guard.valid()) { app.matchTarget = null; return; }
    busy = true;
    const ok = await act(api.applyMatch(item.ratingKey ?? "", r.guid, r.name), `Matched to “${r.name ?? ""}”`, true);
    busy = false;
    if (ok) app.matchTarget = null;
  }

  async function unmatch() {
    const target = item;
    if (!target) return;
    const ok = await app.confirm("Unmatch this item?", {
      text: "Its metadata and artwork are cleared until it's matched again.",
      action: "Unmatch",
      danger: true,
    });
    if (!ok || !guard.valid()) return;
    if (await act(api.unmatch(target.ratingKey ?? ""), "Unmatched", true)) app.matchTarget = null;
  }
</script>

{#if item}
  <dialog class="modal wide" use:modal={close} aria-labelledby="match-title">
    <h2 id="match-title">Fix match</h2>
    <p class="desc">{plex.fullTitle(item)}</p>

    <form
      class="search"
      onsubmit={(e) => {
        e.preventDefault();
        search(true);
      }}
    >
      <input class="input grow" placeholder="Title" bind:value={title} />
      <input class="input year num" placeholder="Year" inputmode="numeric" bind:value={year} />
      <button class="btn" type="submit" disabled={!title.trim()}>Search</button>
    </form>

    {#if error}
      <div class="error-box">{error}</div>
    {:else if results === null}
      <div class="spinner"></div>
    {:else if results.length === 0}
      <p class="faint">No matches. Try a different title or year.</p>
    {:else}
      <div class="rows results">
        {#each results as r, i (r.guid ?? i)}
          <button class="list-row clickable result" disabled={busy} onclick={() => apply(r)}>
            <div class={["thumb", { square }]}><Img src={img(r.thumb, 96, square ? 96 : 144)} /></div>
            <div class="grow">
              <div class="t">
                {r.name}{#if r.year != null}<span class="faint"> ({r.year})</span>{/if}
                {#if r.matched}<span class="tag accent">Current</span>{/if}
              </div>
              {#if r.summary}<p class="sum">{r.summary}</p>{/if}
            </div>
            <div class="end">{r.score != null ? `${r.score}%` : ""}</div>
          </button>
        {/each}
      </div>
    {/if}

    <div class="foot">
      <button class="btn quiet danger unmatch" disabled={busy} onclick={unmatch}>Unmatch</button>
      <button class="btn quiet" disabled={busy} onclick={close}>Cancel</button>
    </div>
  </dialog>
{/if}

<style>
  .search {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }
  .year {
    width: 76px;
  }
  .results {
    max-height: 52vh;
    overflow: auto;
  }
  .result {
    width: 100%;
    align-items: start;
    padding: 10px 4px;
    border-left: 0;
    border-right: 0;
    border-bottom: 0;
    background: none;
    text-align: left;
  }
  .result:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .result .t {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: normal;
  }
  .sum {
    margin-top: 3px;
    color: var(--text-3);
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .result .end {
    padding-top: 1px;
    font-size: 12px;
  }
  .unmatch {
    margin-right: auto;
  }
</style>
