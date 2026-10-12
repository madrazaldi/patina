<script lang="ts">
  // plex.tv watchlist. Each entry links to the server's own copy when there is one.
  import { X } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { act, app, captureScope } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";
  import { untrack } from "svelte";

  const list = resource(() => {
    app.revision;
    return api.watchlist();
  });

  /** How many "is it on this server?" lookups run at once. */
  const LOOKUPS = 5;

  /**
   * guid → what the server said: a rating key, `null` when it doesn't have the title, `undefined` while unknown.
   * A failed lookup is not an answer: it stays unknown (so the entry isn't dimmed) and is retried.
   */
  let local = $state<Record<string, string | null | undefined>>({});
  /** guid → the revision its answer was obtained at; an answer from an older revision is looked up again. */
  const answered = new Map<string, number>();
  /** guid → the revision it was started at, for lookups in flight. */
  const running = new Map<string, number>();
  const queued: string[] = [];
  /** Lookups that failed, as opposed to ones that found the title missing. */
  let failed = $state<Record<string, true>>({});
  let failures = $derived((list.data ?? []).filter((m) => m.guid && failed[m.guid]).length);
  const scope = captureScope();

  function pump() {
    while (running.size < LOOKUPS && queued.length) {
      const guid = queued.shift()!;
      const revision = app.revision;
      running.set(guid, revision);
      api.findByGuid(guid).then(
        (rk) => {
          if (!scope.valid()) return;
          local[guid] = rk ?? null;
          answered.set(guid, revision);
          delete failed[guid];
        },
        () => {
          if (!scope.valid()) return;
          answered.delete(guid);
          failed[guid] = true;
        },
      ).finally(() => {
        running.delete(guid);
        // The library changed while it ran: the answer, or the failure, may be out of date already.
        if (scope.valid() && revision !== app.revision && !queued.includes(guid)) queued.push(guid);
        pump();
      });
    }
  }

  /** Look up every entry whose answer is missing or from before the latest library change. */
  function check(guids: string[]) {
    for (const guid of guids) {
      if (answered.get(guid) === app.revision || running.has(guid) || queued.includes(guid)) continue;
      queued.push(guid);
    }
    pump();
  }

  const guids = $derived([...new Set((list.data ?? []).map((m) => m.guid).filter((g): g is string => !!g))]);
  $effect(() => {
    // Library changes (a scan finishing, a title added or removed) and reconnecting both make old answers unreliable.
    app.revision;
    app.connected;
    const wanted = guids;
    untrack(() => check(wanted));
  });

  function open(m: Metadata) {
    const rk = m.guid ? local[m.guid] : null;
    if (rk) nav.push({ name: "item", ratingKey: rk });
    else if (m.guid && local[m.guid] === undefined) app.toast("Still checking this server");
    else app.toast("Not available on this server");
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Watchlist</h1>
      <div class="sub">Synced with your Plex account. Titles this server doesn't have are dimmed.</div>
    </div>
  </div>
  {#if list.error && list.data !== undefined}<Status error={list.error} retry={() => list.reload()} stale />{/if}
  {#if failures > 0}
    <div class="lookup-failed" role="status">
      Couldn't check {failures === 1 ? "1 title" : `${failures} titles`} against this server, so {failures === 1 ? "it isn't" : "they aren't"} dimmed.
      <button class="btn sm" onclick={() => check(guids)}>Retry</button>
    </div>
  {/if}
  {#if list.data === undefined}
    <Status loading={!list.error} error={list.error} retry={() => list.reload()} />
  {:else if list.data.length === 0}
    <Status title="Your watchlist is empty" text="Add movies and shows from their detail page." />
  {:else}
    <div class="grid">
      {#each list.data as m (m.ratingKey)}
        {@const missing = m.guid != null && local[m.guid] === null}
        <div class="entry" class:missing role="button" tabindex="0" onclick={() => open(m)} onkeydown={(e) => e.key === "Enter" && open(m)}>
          <div class="art">
            <Img src={img(m.thumb, 300, 450)} />
            <button
              class="remove"
              title="Remove from watchlist"
              onclick={(e) => {
                e.stopPropagation();
                act(api.setWatchlisted(m.ratingKey ?? "", false), "Removed from watchlist", true);
              }}><X size={14} /></button
            >
          </div>
          <div class="title ellipsis">{plex.title(m)}</div>
          <div class="sub ellipsis">{missing ? "Not on this server" : plex.subtitle(m)}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .lookup-failed {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
    color: var(--text-2);
  }
  .entry {
    min-width: 0;
    cursor: pointer;
  }
  .entry.missing .art {
    opacity: 0.4;
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
  }
  .art :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .remove {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: rgba(12, 12, 12, 0.82);
    color: var(--text);
    opacity: 0;
    cursor: pointer;
  }
  .entry:hover .remove {
    opacity: 1;
  }
  .title {
    margin-top: 8px;
    font-weight: 500;
  }
  .sub {
    font-size: 12px;
    color: var(--text-3);
  }
</style>
