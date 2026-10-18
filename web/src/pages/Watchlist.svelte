<script lang="ts">
  // plex.tv watchlist. Each entry links to the server's own copy when there is one.
  import { TriangleAlert, X } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { cn } from "$lib/utils";
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
    <Alert.Root class="mb-4" role="status">
      <TriangleAlert />
      <Alert.Description>
        Couldn't check {failures === 1 ? "1 title" : `${failures} titles`} against this server, so {failures === 1 ? "it isn't" : "they aren't"} dimmed.
      </Alert.Description>
      <Alert.Action><Button variant="outline" size="xs" onclick={() => check(guids)}>Retry</Button></Alert.Action>
    </Alert.Root>
  {/if}
  {#if list.data === undefined}
    <Status loading={!list.error} error={list.error} retry={() => list.reload()} />
  {:else if list.data.length === 0}
    <Status title="Your watchlist is empty" text="Add movies and shows from their detail page." />
  {:else}
    <div class="poster-grid">
      {#each list.data as m (m.ratingKey)}
        {@const missing = m.guid != null && local[m.guid] === null}
        <div class="entry group/entry min-w-0 cursor-pointer outline-none" class:missing role="button" tabindex="0" onclick={() => open(m)} onkeydown={(e) => e.key === "Enter" && open(m)}>
          <div
            class={cn(
              "relative aspect-[2/3] overflow-hidden rounded-lg bg-card ring-1 ring-foreground/10 transition-shadow group-hover/entry:ring-foreground/25 group-focus-visible/entry:ring-2 group-focus-visible/entry:ring-tint",
              missing && "opacity-40",
            )}
          >
            <Img src={img(m.thumb, 300, 450)} class="size-full object-cover" />
            <Button
              variant="ghost"
              size="icon-xs"
              class="absolute top-2 right-2 bg-background/80 opacity-0 backdrop-blur-sm group-hover/entry:opacity-100 hover:bg-background focus-visible:opacity-100"
              title="Remove from watchlist"
              onclick={(e) => {
                e.stopPropagation();
                act(api.setWatchlisted(m.ratingKey ?? "", false), "Removed from watchlist", true);
              }}><X /></Button
            >
          </div>
          <div class="mt-2 truncate text-sm leading-5 font-medium">{plex.title(m)}</div>
          <div class="truncate text-xs leading-4 text-muted-foreground">{missing ? "Not on this server" : plex.subtitle(m)}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>
