<script lang="ts">
  import Card from "../components/Card.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { resource } from "../lib/resource.svelte";
  import { app } from "../lib/state.svelte";

  const lists = resource(() => {
    app.revision;
    return api.playlists();
  });
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Playlists</h1>
      <div class="sub">Right-click anything and choose “Add to playlist…” to build one.</div>
    </div>
  </div>
  {#if lists.error && lists.data !== undefined}<Status error={lists.error} retry={() => lists.reload()} stale />{/if}
  {#if lists.data === undefined}
    <Status loading={!lists.error} error={lists.error} retry={() => lists.reload()} />
  {:else if lists.data.length === 0}
    <Status title="No playlists yet" />
  {:else}
    <div class="grid">
      {#each lists.data as item (item.ratingKey)}<Card {item} shape="square" />{/each}
    </div>
  {/if}
</div>
