<script lang="ts">
  // "Fix match" dialog, shown while `app.matchTarget` is set.
  import { untrack } from "svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Badge } from "$lib/components/ui/badge";
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
  import { act, app, captureScope, message } from "../lib/state.svelte";
  import type { MatchResult } from "../lib/types";

  let item = $derived(app.matchTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let title = $state("");
  let year = $state("");
  let results = $state<MatchResult[] | null>(null);
  let error = $state<string>();
  let busy = $state(false);
  let generation = 0;

  let square = $derived(!!item && ["artist", "album"].includes(plex.kind(item)));

  function search(manual: boolean) {
    const target = item;
    if (!target) return;
    const run = ++generation;
    const asked = guard;
    results = null;
    error = undefined;
    const y = Number(year.trim());
    const q = manual ? title.trim() || undefined : undefined;
    api.matches(target.ratingKey ?? "", q, manual && y ? y : undefined).then(
      (r) => run === generation && asked.valid() && (results = r),
      (e) => run === generation && asked.valid() && (error = message(e)),
    );
  }

  // Automatic matches on open.
  $effect(() => {
    const target = item;
    busy = false;
    if (!target) return;
    guard = captureScope();
    title = plex.title(target);
    year = target.year != null ? String(target.year) : "";
    untrack(() => search(false));
  });

  function close() {
    if (!busy) app.matchTarget = null;
  }

  async function apply(r: MatchResult) {
    if (!item || !r.guid || busy) return;
    if (!guard.valid()) { app.matchTarget = null; return; }
    busy = true;
    const ok = await act(api.applyMatch(item.ratingKey ?? "", r.guid, r.name), `Matched to “${r.name ?? ""}”`, true);
    busy = false;
    if (ok) app.matchTarget = null;
  }

  async function unmatch() {
    const target = item;
    if (!target) return;
    const ok = await app.confirm("Unmatch this item?", {
      text: "Its metadata and artwork are cleared until it's matched again.",
      action: "Unmatch",
      danger: true,
    });
    if (!ok || !guard.valid()) return;
    if (await act(api.unmatch(target.ratingKey ?? ""), "Unmatched", true)) app.matchTarget = null;
  }
</script>

{#if item}
  <dialog class="modal wide" use:modal={close} aria-labelledby="match-title">
    <h2 id="match-title">Fix match</h2>
    <p class="desc">{plex.fullTitle(item)}</p>

    <form
      class="mb-4 flex gap-2"
      onsubmit={(e) => {
        e.preventDefault();
        search(true);
      }}
    >
      <Input class="flex-1" placeholder="Title" bind:value={title} />
      <Input class="w-[76px] tabular-nums" placeholder="Year" inputmode="numeric" bind:value={year} />
      <Button variant="outline" type="submit" disabled={!title.trim()}>Search</Button>
    </form>

    {#if error}
      <Alert.Root variant="destructive"><Alert.Description>{error}</Alert.Description></Alert.Root>
    {:else if results === null}
      <div class="flex justify-center py-6"><Spinner class="text-muted-foreground" /></div>
    {:else if results.length === 0}
      <Empty.Root class="border border-dashed p-6">
        <Empty.Header>
          <Empty.Title>No matches</Empty.Title>
          <Empty.Description>No matches. Try a different title or year.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <Item.Group class="max-h-[52vh] gap-1 overflow-auto">
        {#each results as r, i (r.guid ?? i)}
          <Item.Root size="sm" class="hover:bg-muted focus-visible:bg-muted cursor-pointer items-start text-left has-disabled:cursor-default has-disabled:opacity-60 has-disabled:hover:bg-transparent">
            {#snippet child({ props })}
              <button type="button" {...props} disabled={busy} onclick={() => apply(r)}>
                <Item.Media variant="image" class={square ? "" : "h-12 w-8"}>
                  <Img src={img(r.thumb, 96, square ? 96 : 144)} />
                </Item.Media>
                <Item.Content>
                  <Item.Title class="line-clamp-none flex-wrap">
                    {r.name}{#if r.year != null}<span class="text-muted-foreground font-normal"> ({r.year})</span>{/if}
                    {#if r.matched}<Badge variant="secondary">Current</Badge>{/if}
                  </Item.Title>
                  {#if r.summary}<Item.Description class="line-clamp-2 text-xs">{r.summary}</Item.Description>{/if}
                </Item.Content>
                {#if r.score != null}<Item.Actions class="text-muted-foreground text-xs tabular-nums">{r.score}%</Item.Actions>{/if}
              </button>
            {/snippet}
          </Item.Root>
        {/each}
      </Item.Group>
    {/if}

    <div class="foot">
      <Button variant="destructive" class="mr-auto" disabled={busy} onclick={unmatch}>Unmatch</Button>
      <Button variant="outline" disabled={busy} onclick={close}>Cancel</Button>
    </div>
  </dialog>
{/if}
