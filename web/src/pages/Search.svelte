<script lang="ts">
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Label } from "$lib/components/ui/label";
  import { Switch } from "$lib/components/ui/switch";
  import { cn } from "$lib/utils";
  import Card from "../components/Card.svelte";
  import CopyDialog from "../components/CopyDialog.svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { initials } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { browseRoute, nav, routeFor } from "../lib/router.svelte";
  import { app, message } from "../lib/state.svelte";
  import type { AllResults, Directory, Hub, SearchGroup } from "../lib/types";
  import { badge, needsChoice, quality, stateLabel } from "../lib/xsearch";

  let query = $derived(app.search.trim());
  let retry = $state(0);
  let results = $state<{ q: string; hubs?: Hub[]; error?: string } | null>(null);
  /** Results of a search across every server; they grow as servers answer. */
  let everywhere = $state<{ q: string; all?: AllResults; error?: string } | null>(null);
  /** Searching every server needs a second one on the account. */
  let canAll = $derived((app.session?.servers.length ?? 0) > 1);
  let all = $derived(app.searchAll && canAll);
  /** The title whose copies are being chosen. */
  let chosen = $state<SearchGroup | null>(null);

  // Debounced: a newer keystroke cancels the pending search.
  $effect(() => {
    retry;
    let cancelled = false;
    const q = query;
    const wide = all;
    if (q.length < 2) {
      results = null;
      everywhere = null;
      return;
    }
    const timer = setTimeout(() => {
      if (wide) {
        everywhere = { q };
        api
          .searchAllServers(q, (r) => !cancelled && query === q && (everywhere = { q, all: r }))
          .then(
            (r) => !cancelled && query === q && (everywhere = { q, all: r }),
            (e) => !cancelled && query === q && (everywhere = { q, error: message(e) }),
          );
        return;
      }
      api.search(q).then(
        (hubs) => !cancelled && query === q && (results = { q, hubs: hubs.filter((h) => h.Metadata.length || h.Directory.length) }),
        (e) => !cancelled && query === q && (results = { q, error: message(e) }),
      );
    }, 220);
    return () => { cancelled = true; clearTimeout(timer); };
  });

  function openGroup(g: SearchGroup) {
    if (needsChoice(g)) chosen = g;
    else nav.push(routeFor(g.copies[0].item));
  }

  const people = ["actor", "director", "writer", "producer", "person", "tag"];
  const openTag = (d: Directory) => d.key && nav.push(browseRoute(d.key, d.title ?? d.tag ?? ""));
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>{query ? `“${query}”` : "Search"}</h1>
      <div class="sub">{all ? "Every library on every server of your account." : "Every library on this server."} Press / anywhere to search.</div>
    </div>
    {#if canAll}
      <div class="flex items-center gap-2.5">
        <Switch id="search-all-servers" checked={app.searchAll} onCheckedChange={(v) => (app.searchAll = v)} />
        <Label for="search-all-servers" class="cursor-pointer text-muted-foreground">Search all servers</Label>
      </div>
    {/if}
  </div>

  {#if all && query.length >= 2}
    {#if !everywhere || everywhere.q !== query}
      <Status loading />
    {:else if everywhere.error}
      <Status error={everywhere.error} retry={() => retry++} />
    {:else}
      {@const found = everywhere.all}
      {#if found}
        <ul class="mb-4 flex flex-wrap gap-x-4 gap-y-1.5 text-xs text-muted-foreground" aria-label="Servers searched" role="status">
          {#each found.servers as s (s.id)}
            <li class={["srv", s.state, (s.state === "failed" || s.state === "timedOut") && "text-destructive"]}><b class={["font-medium", s.state !== "failed" && s.state !== "timedOut" && "text-foreground/80"]}>{s.name}</b> {stateLabel(s)}</li>
          {/each}
        </ul>
        {#if found.groups.length}
          <div class="rows">
            {#each found.groups as g (g.key)}
              {@const first = g.copies[0]}
              <button class="list-row clickable group w-full text-left" onclick={() => openGroup(g)} aria-label={`${g.title}${g.year ? ` (${g.year})` : ""}, on ${g.copies.map(badge).join(", ")}`}>
                <div class="thumb"><Img src={img(plex.poster(first.item), 96, 144, first.item.patinaServer)} /></div>
                <div class="grow">
                  <div class="t">{g.title}{#if g.year}<span class="ml-1 text-muted-foreground">({g.year})</span>{/if}</div>
                  <div class="s">{g.subtitle ?? g.kind}</div>
                </div>
                <div class="end max-w-1/2 flex-wrap justify-end">
                  {#each g.copies as c (`${c.serverId}:${c.item.ratingKey}`)}
                    <Badge variant="outline" class={cn(c.current && "border-tint/40 bg-tint/10 text-tint")} title={c.versions.map((v) => v.label).join("\n")}>{c.serverName}{#if quality(c)} <span class="tabular-nums">{quality(c)}</span>{/if}</Badge>
                  {/each}
                </div>
              </button>
            {/each}
          </div>
        {:else if found.done}
          <Status title="No results" text="None of your servers has it. Check the spelling or try another word." />
        {:else}
          <Status loading />
        {/if}
      {:else}
        <Status loading />
      {/if}
    {/if}
  {:else if query.length < 2}
    <Status title="Find something to watch" text="Movies, shows, episodes, music, actors, directors, genres…" />
  {:else if !results || results.q !== query}
    <Status loading />
  {:else if results.error}
    <Status error={results.error} retry={() => retry++} />
  {:else if !results.hubs?.length}
    <Status title="No results" text="Check the spelling or try another word." />
  {:else}
    {#each results.hubs as hub, i (`${hub.hubIdentifier}:${i}`)}
      <section>
        <div class="section-head"><h2>{hub.title}</h2></div>
        {#if hub.Metadata.length}
          {@const wide = hub.type === "episode" || hub.type === "clip"}
          <div class={["poster-grid", { wide }]}>
            {#each hub.Metadata as item, j (`${item.ratingKey}:${j}`)}
              <Card {item} shape={wide ? "landscape" : undefined} />
            {/each}
          </div>
        {/if}
        {#if hub.Directory.length}
          <div class="flex flex-wrap gap-x-2.5 gap-y-4">
            {#each hub.Directory as d, j (`${d.key}:${d.tag}:${j}`)}
              {@const name = d.title ?? d.tag ?? ""}
              {#if people.includes(d.type ?? "")}
                <button class="group/person flex w-24 cursor-pointer flex-col items-center gap-0.5 text-center text-xs" onclick={() => openTag(d)}>
                  <span class="mb-1.5 grid size-18 place-items-center overflow-hidden rounded-full bg-muted font-semibold text-muted-foreground ring-1 ring-foreground/10 transition-shadow group-hover/person:ring-foreground/30">
                    {#if d.thumb}<Img src={img(d.thumb, 160, 160)} class="size-full object-cover" />{:else}{initials(name)}{/if}
                  </span>
                  <span class="font-medium underline-offset-3 group-hover/person:underline">{name}</span>
                  {#if d.count != null}<span class="text-muted-foreground">{d.count} titles</span>{/if}
                </button>
              {:else}
                <Button variant="outline" size="sm" class="rounded-full" onclick={() => openTag(d)}>{name}</Button>
              {/if}
            {/each}
          </div>
        {/if}
      </section>
    {/each}
  {/if}
</div>

{#if chosen}
  <CopyDialog group={chosen} onclose={() => (chosen = null)} />
{/if}
