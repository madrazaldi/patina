<script lang="ts">
  // "Add to playlist" dialog, shown while `app.playlistTarget` is set.
  import { Plus } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import { Input } from "$lib/components/ui/input";
  import * as Item from "$lib/components/ui/item";
  import { Spinner } from "$lib/components/ui/spinner";
  import Img from "./Img.svelte";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { act, app, captureScope } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let item = $derived(app.playlistTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let lists = $state<Metadata[] | null>(null);
  let error = $state<string>();
  let name = $state("");

  $effect(() => {
    const target = item;
    lists = null;
    error = undefined;
    name = "";
    if (!target) return;
    const opened = (guard = captureScope());
    api.playlists(plex.playlistKind(target)).then(
      (l) => app.playlistTarget === target && opened.valid() && (lists = l),
      (e) => app.playlistTarget === target && opened.valid() && (error = String(e)),
    );
  });

  const close = () => { app.playlistTarget = null; app.playlistItems = []; };

  function add(target: { playlist?: string; title?: string }, ok: string) {
    if (!item || !guard.valid()) return close();
    const rk = app.playlistItems.length ? app.playlistItems.map((m) => m.ratingKey).join(",") : item.ratingKey ?? "";
    const kind = plex.playlistKind(item);
    close();
    act(api.addToPlaylist(rk, kind, target), ok);
  }

  function create() {
    const title = name.trim();
    if (title) add({ title }, `Created playlist “${title}”`);
  }
</script>

{#if item}
  <dialog class="modal" use:modal={close} aria-labelledby="playlist-title">
    <h2 id="playlist-title">Add to playlist</h2>
    <p class="desc">{app.playlistItems.length ? `${app.playlistItems.length} selected titles` : plex.fullTitle(item)}</p>

    {#if error}
      <Alert.Root variant="destructive"><Alert.Description>{error}</Alert.Description></Alert.Root>
    {:else if lists === null}
      <div class="flex justify-center py-6"><Spinner class="text-muted-foreground" /></div>
    {:else if lists.length === 0}
      <Empty.Root class="border border-dashed p-6">
        <Empty.Header>
          <Empty.Title>No playlists yet</Empty.Title>
          <Empty.Description>No playlists of this kind yet.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <Item.Group role="group" aria-label="Playlists" class="max-h-[300px] gap-1 overflow-auto">
        {#each lists as p (p.ratingKey)}
          <Item.Root size="sm" class="hover:bg-muted focus-visible:bg-muted cursor-pointer text-left">
            {#snippet child({ props })}
              <button type="button" {...props} onclick={() => add({ playlist: p.ratingKey }, `Added to “${p.title}”`)}>
                <Item.Media variant="image">
                  <Img src={img(p.composite ?? p.thumb, 96, 96)} />
                </Item.Media>
                <Item.Content>
                  <Item.Title>{p.title}</Item.Title>
                  <Item.Description>{plex.subtitle(p)}</Item.Description>
                </Item.Content>
                <Item.Actions><Plus class="text-muted-foreground size-4" /></Item.Actions>
              </button>
            {/snippet}
          </Item.Root>
        {/each}
      </Item.Group>
    {/if}

    <div class="mt-4 flex gap-2">
      <Input
        class="flex-1"
        placeholder="New playlist"
        bind:value={name}
        onkeydown={(e) => e.key === "Enter" && create()}
      />
      <Button variant="outline" disabled={!name.trim()} onclick={create}>Create</Button>
    </div>
    <div class="foot"><Button variant="outline" onclick={close}>Cancel</Button></div>
  </dialog>
{/if}
