<script lang="ts">
  import { ChevronLeft, ChevronRight, Play } from "@lucide/svelte";
  import HubRow from "../components/HubRow.svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { play } from "../lib/actions.svelte";
  import { api } from "../lib/api";
  import { duration } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav, routeFor } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";
  import type { Hub, Metadata } from "../lib/types";

  const hubs = resource(() => {
    app.revision;
    return api.homeHubs();
  });

  let rows = $derived((hubs.data ?? []).filter((h) => h.Metadata.length || h.Directory.length));

  /** Featured items: recent or in-progress movies and shows with backdrop art. */
  function featured(list: Hub[]) {
    const out: [Metadata, string][] = [];
    for (const hub of list) {
      const id = (hub.hubIdentifier ?? "").toLowerCase();
      const label = /continue|ondeck/.test(id) ? "Continue watching" : id.includes("recent") ? "Recently added" : null;
      if (!label) continue;
      for (const m of hub.Metadata) {
        const k = plex.kind(m);
        if (out.length < 6 && m.art && (k === "movie" || k === "show") && !out.some(([x]) => x.ratingKey === m.ratingKey))
          out.push([m, label]);
      }
    }
    return out;
  }

  let picks = $derived(featured(rows));
  let index = $state(0);
  let current = $derived(picks.length ? picks[index % picks.length] : null);
  const pad = (n: number) => String(n).padStart(2, "0");
</script>

{#if hubs.data === undefined}
  <Status loading={!hubs.error} error={hubs.error} fill />
{:else if rows.length === 0}
  <Status title="Nothing here yet" text="Your libraries are empty or still scanning." fill />
{:else}
  {#if current}
    {@const [item, label] = current}
    {@const meta = [
      item.year,
      item.contentRating,
      plex.kind(item) === "show"
        ? item.childCount != null && `${item.childCount} season${item.childCount === 1 ? "" : "s"}`
        : item.duration != null && duration(item.duration),
      item.Genre.slice(0, 2)
        .map((g) => g.tag)
        .join(", "),
    ].filter(Boolean)}
    <section class="hero">
      {#key item.ratingKey}
        <div class="backdrop"><Img src={img(item.art, 1920, 1080)} /></div>
      {/key}
      <div class="copy">
        <div class="kicker">{label}</div>
        <h1 class="serif">{plex.title(item)}</h1>
        <div class="meta">{meta.join("  ·  ")}</div>
        {#if item.summary}<p class="summary">{item.summary}</p>{/if}
        <div class="actions">
          <button class="btn primary lg" onclick={() => play(item)}>
            <Play size={14} fill="currentColor" />{plex.progress(item) != null ? "Resume" : "Play"}
          </button>
          <button class="btn lg" onclick={() => nav.push(routeFor(item))}>Details</button>
        </div>
      </div>
      {#if picks.length > 1}
        <div class="pager num">
          <button class="icon-btn" title="Previous" onclick={() => (index = (index + picks.length - 1) % picks.length)}
            ><ChevronLeft /></button
          >
          <span>{pad((index % picks.length) + 1)} / {pad(picks.length)}</span>
          <button class="icon-btn" title="Next" onclick={() => (index = (index + 1) % picks.length)}><ChevronRight /></button>
        </div>
      {/if}
    </section>
  {/if}
  <div class="page hubs" class:flush={!current}>
    {#each rows as hub, i (`${hub.hubIdentifier}:${hub.key}:${i}`)}
      <HubRow {hub} />
    {/each}
  </div>
{/if}

<style>
  .hero {
    position: relative;
    height: min(56vh, 560px);
    min-height: 360px;
    overflow: hidden;
    border-bottom: 1px solid var(--line);
  }
  .backdrop {
    position: absolute;
    inset: 0;
  }
  .backdrop :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 20%;
  }
  /* Legibility scrim for the copy on the left; not decoration. */
  .hero::after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(12, 12, 12, 0.92) 0%, rgba(12, 12, 12, 0.55) 45%, rgba(12, 12, 12, 0) 75%),
      linear-gradient(0deg, rgba(12, 12, 12, 0.7) 0%, rgba(12, 12, 12, 0) 40%);
  }
  .copy {
    position: absolute;
    z-index: 1;
    left: var(--gutter);
    bottom: 40px;
    max-width: 560px;
  }
  .kicker {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  h1 {
    margin-top: 10px;
    font-size: 48px;
    font-weight: 400;
    line-height: 1.05;
    letter-spacing: -0.015em;
  }
  .meta {
    margin-top: 12px;
    color: var(--text-2);
  }
  .summary {
    margin-top: 14px;
    color: var(--text-2);
    font-size: 14px;
    line-height: 1.55;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 22px;
  }
  .pager {
    position: absolute;
    z-index: 1;
    right: calc(var(--gutter) - 8px);
    bottom: 34px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
  }
  .hubs {
    padding-top: 4px;
  }
  .hubs.flush {
    padding-top: 0;
  }
</style>
