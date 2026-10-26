<script lang="ts">
  // Seerr: discover titles to request, and follow (or manage) requests.
  import { ChevronLeft, ChevronRight, Inbox } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import SeerrCard from "../components/SeerrCard.svelte";
  import SeerrRequestRow from "../components/SeerrRequestRow.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { seerrStatus } from "../lib/seerr";
  import { app, message } from "../lib/state.svelte";
  import type { SeerrStatus, SeerrTitle } from "../lib/types";

  let tab = $state<"discover" | "requests">("discover");

  let status = $state<SeerrStatus | null>(null);
  let statusError = $state<string>();
  $effect(() => {
    const config = app.seerr;
    status = null;
    statusError = undefined;
    if (config) seerrStatus(config.url).then((s) => (status = s), (e) => (statusError = message(e)));
  });

  const LISTS = [
    ["trending", "Trending"],
    ["movies", "Popular movies"],
    ["tv", "Popular shows"],
    ["upcoming-movies", "Upcoming movies"],
    ["upcoming-tv", "Upcoming shows"],
  ] as const;
  let list = $state<string>("trending");

  // Discover pages accumulate with "Load more"; switching lists starts over.
  let titles = $state<SeerrTitle[]>([]);
  let shown = $state({ list: "", page: 0, pages: 0 });
  let loading = $state(false);
  let discoverError = $state<string>();
  let generation = 0;

  async function load(which: string, page: number) {
    const mine = ++generation;
    loading = true;
    discoverError = undefined;
    try {
      const got = await api.seerrDiscover(which, page);
      if (mine !== generation) return;
      // TMDB pages can overlap as popularity shifts.
      const seen = new Set(page === 1 ? [] : titles.map((t) => `${t.mediaType}:${t.tmdbId}`));
      const fresh = got.results.filter((t) => !seen.has(`${t.mediaType}:${t.tmdbId}`));
      titles = page === 1 ? fresh : [...titles, ...fresh];
      shown = { list: which, page: got.page, pages: got.pages };
    } catch (e) {
      if (mine === generation) discoverError = message(e);
    } finally {
      if (mine === generation) loading = false;
    }
  }

  $effect(() => {
    if (tab === "discover" && app.seerr && shown.list !== list) {
      titles = [];
      load(list, 1);
    }
  });

  const FILTERS = [
    ["all", "All"],
    ["pending", "Pending approval"],
    ["approved", "Approved"],
    ["processing", "Processing"],
    ["available", "Available"],
    ["failed", "Failed"],
  ] as const;
  let filter = $state("all");
  let page = $state(1);
  const requests = resource(() => {
    const f = filter, p = page;
    return tab === "requests" && app.seerr ? api.seerrRequests(f, p) : Promise.resolve(null);
  });
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Requests</h1>
      <div class="sub">
        {#if status}Seerr at {status.url} · signed in as {status.user.name}{:else if app.seerr}Seerr at {app.seerr.url}{:else}Request movies and shows the server doesn't have.{/if}
      </div>
    </div>
    {#if app.seerr}
      <div class="tabs">
        <button class={["tab", { active: tab === "discover" }]} onclick={() => (tab = "discover")}>Discover</button>
        <button class={["tab", { active: tab === "requests" }]} onclick={() => (tab = "requests")}>{status?.can.manage ? "All requests" : "My requests"}</button>
      </div>
    {/if}
  </div>

  {#if !app.seerr}
    <Empty.Root class="border">
      <Empty.Header>
        <Empty.Media variant="icon"><Inbox /></Empty.Media>
        <Empty.Title>Seerr is not set up</Empty.Title>
        <Empty.Description>Connect a Seerr, Overseerr or Jellyseerr server in Settings to request titles.</Empty.Description>
      </Empty.Header>
      <Empty.Content><Button variant="outline" onclick={() => nav.push({ name: "settings" })}>Open Settings</Button></Empty.Content>
    </Empty.Root>
  {:else}
    {#if statusError}<Status error={statusError} stale />{/if}
    {#if tab === "discover"}
      <ToggleGroup.Root type="single" variant="outline" size="sm" class="mb-5" bind:value={() => list, (v) => v && (list = v)} aria-label="List">
        {#each LISTS as [id, label] (id)}<ToggleGroup.Item value={id}>{label}</ToggleGroup.Item>{/each}
      </ToggleGroup.Root>
      {#if discoverError && !titles.length}
        <Status error={discoverError} retry={() => load(list, 1)} />
      {:else if !titles.length}
        <Status loading={loading} title="Nothing here" text="Seerr returned no titles for this list." />
      {:else}
        <div class="poster-grid">
          {#each titles as t (`${t.mediaType}:${t.tmdbId}`)}<SeerrCard title={t} />{/each}
        </div>
        {#if discoverError}<p class="text-destructive mt-4 text-center text-sm">{discoverError}</p>{/if}
        {#if shown.page < shown.pages}
          <div class="mt-6 flex justify-center">
            <Button variant="outline" disabled={loading} onclick={() => load(list, shown.page + 1)}>{loading ? "Loading…" : "Load more"}</Button>
          </div>
        {/if}
      {/if}
    {:else}
      <div class="mb-4 flex items-center gap-3">
        <NativeSelect.Root
          class="w-52"
          aria-label="Show requests"
          value={filter}
          onchange={(e) => {
            filter = e.currentTarget.value;
            page = 1;
          }}
        >
          {#each FILTERS as [id, label] (id)}<NativeSelect.Option value={id}>{label}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </div>
      {@const r = requests.data}
      {#if requests.error && r}<Status error={requests.error} retry={() => requests.reload()} stale />{/if}
      {#if !r}
        <Status loading={!requests.error} error={requests.error} retry={() => requests.reload()} />
      {:else if r.results.length === 0}
        <Status title="No requests" text={filter === "all" ? "Requests made in Patina or on the Seerr website show up here." : "No requests match this filter."} />
      {:else}
        <div class="rows">
          {#each r.results as req (req.id)}
            <SeerrRequestRow request={req} can={status?.can ?? null} onchange={() => requests.reload()} />
          {/each}
        </div>
        {#if r.pages > 1}
          <div class="mt-5 flex items-center justify-center gap-3.5">
            <Button variant="ghost" size="sm" disabled={page <= 1} onclick={() => page--}><ChevronLeft />Newer</Button>
            <span class="text-muted-foreground text-sm tabular-nums">Page {r.page} of {r.pages}</span>
            <Button variant="ghost" size="sm" disabled={page >= r.pages} onclick={() => page++}>Older<ChevronRight /></Button>
          </div>
        {/if}
      {/if}
    {/if}
  {/if}
</div>
