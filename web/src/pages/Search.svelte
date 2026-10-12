<script lang="ts">
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
      <label class="scope">
        <input type="checkbox" checked={app.searchAll} onchange={(e) => (app.searchAll = e.currentTarget.checked)} />
        Search all servers
      </label>
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
        <ul class="servers" aria-label="Servers searched" role="status">
          {#each found.servers as s (s.id)}
            <li class={["srv", s.state]}><b>{s.name}</b> {stateLabel(s)}</li>
          {/each}
        </ul>
        {#if found.groups.length}
          <div class="rows">
            {#each found.groups as g (g.key)}
              {@const first = g.copies[0]}
              <button class="list-row clickable group" onclick={() => openGroup(g)} aria-label={`${g.title}${g.year ? ` (${g.year})` : ""}, on ${g.copies.map(badge).join(", ")}`}>
                <div class="thumb"><Img src={img(plex.poster(first.item), 96, 144, first.item.patinaServer)} /></div>
                <div class="grow">
                  <div class="t">{g.title}{#if g.year} <span class="faint">({g.year})</span>{/if}</div>
                  <div class="s">{g.subtitle ?? g.kind}</div>
                </div>
                <div class="end copies">
                  {#each g.copies as c (c.serverId)}
                    <span class={["tag", { accent: c.current }]} title={c.versions.map((v) => v.label).join("\n")}>{c.serverName}{#if quality(c)} <span class="num">{quality(c)}</span>{/if}</span>
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
          <div class={["grid", { wide }]}>
            {#each hub.Metadata as item, j (`${item.ratingKey}:${j}`)}
              <Card {item} shape={wide ? "landscape" : undefined} />
            {/each}
          </div>
        {/if}
        {#if hub.Directory.length}
          <div class="tags">
            {#each hub.Directory as d, j (`${d.key}:${d.tag}:${j}`)}
              {@const name = d.title ?? d.tag ?? ""}
              {#if people.includes(d.type ?? "")}
                <button class="person" onclick={() => openTag(d)}>
                  <span class="face">
                    {#if d.thumb}<Img src={img(d.thumb, 160, 160)} />{:else}{initials(name)}{/if}
                  </span>
                  <span class="name">{name}</span>
                  {#if d.count != null}<span class="faint">{d.count} titles</span>{/if}
                </button>
              {:else}
                <button class="chip" onclick={() => openTag(d)}>{name}</button>
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

<style>
  .scope {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .servers {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
    font-size: 12px;
    color: var(--text-3);
  }
  .srv b {
    font-weight: 500;
    color: var(--text-2);
  }
  .srv.failed,
  .srv.timedOut {
    color: #e0a89f;
  }
  .group {
    width: 100%;
    border-left: 0;
    border-right: 0;
    border-bottom: 0;
    background: none;
    color: inherit;
    text-align: left;
  }
  .copies {
    flex-wrap: wrap;
    justify-content: flex-end;
    max-width: 50%;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 18px 10px;
  }
  .person {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    width: 96px;
    padding: 0;
    border: 0;
    background: none;
    font-size: 12px;
    text-align: center;
    cursor: pointer;
  }
  .face {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    margin-bottom: 5px;
    border-radius: 50%;
    overflow: hidden;
    background: var(--surface);
    color: var(--text-3);
    font-weight: 600;
  }
  .face :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .person:hover .name {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
