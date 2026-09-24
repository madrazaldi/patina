<script lang="ts">
  import Card from "../components/Card.svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { initials } from "../lib/format";
  import { img } from "../lib/images";
  import { browseRoute, nav } from "../lib/router.svelte";
  import { app, message } from "../lib/state.svelte";
  import type { Directory, Hub } from "../lib/types";

  let query = $derived(app.search.trim());
  let results = $state<{ q: string; hubs?: Hub[]; error?: string } | null>(null);

  // Debounced: a newer keystroke cancels the pending search.
  $effect(() => {
    const q = query;
    if (q.length < 2) {
      results = null;
      return;
    }
    const timer = setTimeout(() => {
      api.search(q).then(
        (hubs) => query === q && (results = { q, hubs: hubs.filter((h) => h.Metadata.length || h.Directory.length) }),
        (e) => query === q && (results = { q, error: message(e) }),
      );
    }, 220);
    return () => clearTimeout(timer);
  });

  const people = ["actor", "director", "writer", "producer", "person", "tag"];
  const openTag = (d: Directory) => d.key && nav.push(browseRoute(d.key, d.title ?? d.tag ?? ""));
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>{query ? `“${query}”` : "Search"}</h1>
      <div class="sub">Every library on this server. Press / anywhere to search.</div>
    </div>
  </div>

  {#if query.length < 2}
    <Status title="Find something to watch" text="Movies, shows, episodes, music, actors, directors, genres…" />
  {:else if !results || results.q !== query}
    <Status loading />
  {:else if results.error}
    <Status error={results.error} />
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

<style>
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
