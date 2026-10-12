<script lang="ts">
  // Set audio and subtitle tracks for many episodes at once. The matching is
  // done in Rust (crates/app/src/tracks.rs); this dialog only collects choices.
  import { TriangleAlert } from "@lucide/svelte";
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { app, message } from "../lib/state.svelte";
  import type { Metadata, TrackView } from "../lib/types";

  let item = $derived(app.tracksTarget);

  let loaded = $state(false);
  let error = $state<string>();
  let season = $state<number | null>(null);
  let reference = $state<string | null>(null);
  let audio = $state<number | null>(null);
  let subtitle = $state("keep");
  /** Also save the choices as this show's playback preferences. */
  let remember = $state(false);
  let view = $state<TrackView>();
  let progress = $state<[number, number] | null>(null);
  let busy = $derived(progress !== null);

  function showOf(m: Metadata): [string | undefined, string] {
    switch (plex.kind(m)) {
      case "show":
        return [m.ratingKey, plex.title(m)];
      case "season":
        return [m.parentRatingKey, m.parentTitle ?? ""];
      default:
        return [m.grandparentRatingKey, m.grandparentTitle ?? ""];
    }
  }

  let show = $derived(item ? showOf(item) : [undefined, ""]);

  // Load the show's episodes whenever the dialog opens.
  $effect(() => {
    const target = item;
    loaded = false;
    error = undefined;
    view = undefined;
    progress = null;
    if (!target) return;
    const k = plex.kind(target);
    season = k === "season" ? (target.index ?? null) : k === "episode" ? (target.parentIndex ?? null) : null;
    reference = k === "episode" ? (target.ratingKey ?? null) : null;
    audio = null;
    subtitle = "keep";
    remember = false;
    const [rk] = showOf(target);
    if (!rk) {
      error = "This item doesn't belong to a show.";
      return;
    }
    api.tracksLoad(rk).then(
      () => app.tracksTarget === target && (loaded = true),
      (e) => (error = message(e)),
    );
  });

  // Re-plan whenever a choice changes.
  $effect(() => {
    const rk = show[0];
    if (!loaded || !rk) return;
    const query = { show: rk, season, reference, audio, subtitle };
    api.tracksView(query).then(
      (v) => (view = v),
      (e) => (error = message(e)),
    );
  });

  function close() {
    if (!busy) app.tracksTarget = null;
  }

  function resetTracks() {
    audio = null;
    subtitle = "keep";
  }

  let canApply = $derived(!!view && (view.plan.changes.length > 0 || (remember && !view.nothingChosen)));

  async function apply() {
    if (!view) return;
    const changes = view.plan.changes;
    progress = [0, changes.length];
    try {
      // Before applying: applying drops the loaded tracks the rules are read from.
      if (remember && show[0]) await api.tracksRemember({ show: show[0], season, reference, audio, subtitle });
      if (changes.length) await api.tracksApply(changes, (done, total) => (progress = [done, total]));
    } catch (e) {
      app.error(e);
    }
    progress = null;
    app.tracksTarget = null;
  }

  const epCount = (n: number) => `${n} episode${n === 1 ? "" : "s"}`;
  const missHead = (n: number, what: string) => `${epCount(n)} ${n === 1 ? "has" : "have"} ${what}, left unchanged`;
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && item && close()} />

{#if item}
  <div class="modal-backdrop" role="presentation" onclick={close}>
    <div class="modal wide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>Audio & subtitles</h2>
      <p class="desc">{show[1]}: pick tracks in one episode and Patina selects the closest match in every other.</p>

      {#if error}
        <div class="error-box">{error}</div>
      {:else if !view}
        <div class="loading">
          <div class="spinner"></div>
          <span class="faint">Reading tracks from every episode…</span>
        </div>
      {:else if view.total === 0}
        <p class="faint">This show has no episodes.</p>
      {:else}
        <div class="fields">
          <label>
            <span>Apply to</span>
            <select
              class="select"
              disabled={busy}
              value={season === null ? "all" : String(season)}
              onchange={(e) => {
                const v = e.currentTarget.value;
                season = v === "all" ? null : Number(v);
                reference = null;
                resetTracks();
              }}
            >
              <option value="all">All seasons ({epCount(view.total)})</option>
              {#each view.seasons as s (s.index)}
                <option value={String(s.index)}>{s.title} ({epCount(s.count)})</option>
              {/each}
            </select>
          </label>
          <label>
            <span>Take tracks from</span>
            <select
              class="select"
              disabled={busy}
              value={view.reference ?? ""}
              onchange={(e) => {
                reference = e.currentTarget.value;
                resetTracks();
              }}
            >
              {#each view.scope as ep (ep.ratingKey)}
                <option value={ep.ratingKey}>{ep.label}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>Audio</span>
            <select
              class="select"
              disabled={busy || view.audio.length === 0}
              value={audio === null ? "keep" : String(audio)}
              onchange={(e) => {
                const v = e.currentTarget.value;
                audio = v === "keep" ? null : Number(v);
              }}
            >
              <option value="keep">Don't change</option>
              {#each view.audio as s (s.id)}
                <option value={String(s.id)}>{s.label}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>Subtitles</span>
            <select class="select" disabled={busy} bind:value={subtitle}>
              <option value="keep">Don't change</option>
              <option value="off">None (turn off)</option>
              {#each view.subtitles as s (s.id)}
                <option value={String(s.id)}>{s.label}{s.external ? " · external" : ""}</option>
              {/each}
            </select>
          </label>
        </div>

        <label class="remember">
          <input type="checkbox" bind:checked={remember} disabled={busy || view.nothingChosen} />
          <span>
            Remember for this show
            <small>Use these languages for new episodes too, and whenever the show starts playing.</small>
          </span>
        </label>

        <div class="summary">
          {#if progress}
            <div class="bar"><i style:width="{(progress[0] / Math.max(1, progress[1])) * 100}%"></i></div>
            <span class="num">Updating {progress[0]} of {progress[1]}…</span>
          {:else if view.nothingChosen}
            <span class="faint">
              Choose an audio or subtitle track. Matches are found by language first, then title, codec and channels.
            </span>
          {:else if view.plan.episodes === 0 && !view.plan.noAudio.length && !view.plan.noSubtitle.length}
            <span>Every episode already uses these tracks.</span>
          {:else}
            <span>{view.plan.episodes} of {epCount(view.scope.length)} will change.</span>
          {/if}
          {#if !busy && view.plan.noAudio.length}
            <details>
              <summary><TriangleAlert size={14} />{missHead(view.plan.noAudio.length, `no audio matching “${view.audioName}”`)}</summary>
              <ul>{#each view.plan.noAudio as e (e)}<li>{e}</li>{/each}</ul>
            </details>
          {/if}
          {#if !busy && view.plan.noSubtitle.length}
            <details>
              <summary>
                <TriangleAlert size={14} />{missHead(view.plan.noSubtitle.length, `no subtitles matching “${view.subtitleName}”`)}
              </summary>
              <ul>{#each view.plan.noSubtitle as e (e)}<li>{e}</li>{/each}</ul>
            </details>
          {/if}
        </div>
      {/if}

      <div class="foot">
        <button class="btn quiet" disabled={busy} onclick={close}>Cancel</button>
        <button class="btn primary" disabled={busy || !canApply} onclick={apply}>
          {view && view.plan.episodes > 0 ? `Apply to ${epCount(view.plan.episodes)}` : remember && canApply ? "Remember" : "Apply"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 40px 0;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px 16px;
  }
  label span {
    display: block;
    margin-bottom: 5px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  label .select {
    width: 100%;
  }
  label.remember {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    margin-top: 16px;
    cursor: pointer;
  }
  label.remember input {
    margin-top: 2px;
  }
  label.remember span {
    margin: 0;
    font-size: 13px;
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-1, inherit);
  }
  label.remember small {
    display: block;
    margin-top: 2px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .summary {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }
  .summary .bar {
    height: 2px;
  }
  details {
    color: var(--text-2);
  }
  summary {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    list-style: none;
  }
  summary :global(svg) {
    color: #d6a44c;
  }
  ul {
    margin: 8px 0 0;
    padding-left: 21px;
    max-height: 140px;
    overflow: auto;
    color: var(--text-3);
    font-size: 12px;
  }
</style>
