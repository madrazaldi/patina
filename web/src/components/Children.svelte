<script lang="ts">
  // Seasons, episodes, albums, tracks or collection items below a detail header.
  import { Play, X } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import Card from "./Card.svelte";
  import Img from "./Img.svelte";
  import Row from "./Row.svelte";
  import Status from "./Status.svelte";
  import { openMenu, play } from "../lib/actions.svelte";
  import { api } from "../lib/api";
  import { airDate, clock, duration } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav, routeFor } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let { item, sort }: { item: Metadata; /** Collection sort: 2 (or "2") is custom. */ sort?: string | number } = $props();

  let kind = $derived(plex.kind(item));
  const children = resource(() => {
    app.revision;
    return api.children(item.ratingKey ?? "", kind === "collection", item.patinaServer);
  });

  const open = (m: Metadata) => nav.push(routeFor(m));

  // Collection items; reassigned optimistically on remove / reorder until the refetch lands.
  let items = $derived(children.data ?? []);
  let editable = $derived(kind === "collection" && app.owned && !item.patinaServer);
  let reorderable = $derived(editable && String(sort) === "2" && items.length > 1);

  function removeItem(m: Metadata) {
    const rk = m.ratingKey ?? "";
    items = items.filter((x) => x.ratingKey !== rk);
    act(api.removeFromCollection(item.ratingKey ?? "", rk), "Removed from collection", true);
  }

  // Pointer drag rather than HTML5 DnD, which the Tauri webview swallows on Windows.
  let grid: HTMLDivElement | undefined = $state();
  let drag = $state<{ rk: string; x: number; y: number; moved: boolean } | null>(null);
  /** Item the dragged one would land before; null is the end. */
  let before = $state<string | null | undefined>(undefined);
  let dropped = false;

  function down(e: PointerEvent, rk: string) {
    if (!reorderable || e.button !== 0) return;
    drag = { rk, x: e.clientX, y: e.clientY, moved: false };
  }

  function move(e: PointerEvent) {
    if (!drag || !grid) return;
    if (!drag.moved) {
      if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 6) return;
      drag.moved = true;
      // Capture only once dragging, so plain clicks still reach the card.
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    }
    const cells = [...grid.querySelectorAll<HTMLElement>("[data-rk]")];
    const hit = cells.find((c) => {
      const b = c.getBoundingClientRect();
      return e.clientY < b.top || (e.clientY <= b.bottom && e.clientX < b.left + b.width / 2);
    });
    before = hit ? (hit.dataset.rk ?? null) : null;
  }

  function up() {
    const d = drag;
    if (d?.moved && before !== undefined && before !== d.rk) {
      const dragged = items.find((m) => m.ratingKey === d.rk);
      const rest = items.filter((m) => m.ratingKey !== d.rk);
      const at = before === null ? rest.length : rest.findIndex((m) => m.ratingKey === before);
      const after = at > 0 ? (rest[at - 1].ratingKey ?? null) : null;
      if (dragged && at >= 0 && items.indexOf(dragged) !== at) {
        items = [...rest.slice(0, at), dragged, ...rest.slice(at)];
        act(api.moveCollectionItem(item.ratingKey ?? "", d.rk, after), undefined, true);
      }
    }
    if (d?.moved) {
      dropped = true;
      setTimeout(() => (dropped = false));
    }
    drag = null;
    before = undefined;
  }
</script>

{#if children.data === undefined}
  <Status loading={!children.error} error={children.error} />
{:else if children.data.length}
  {@const list = children.data}
  {#if kind === "show"}
    <Row title="Seasons" items={list} />
  {:else if kind === "season"}
    <div class="section-head"><h2>Episodes</h2></div>
    <div class="rows">
      {#each list as ep (ep.ratingKey)}
        {@const spoiler = app.spoilers && !plex.isWatched(ep)}
        {@const p = plex.progress(ep)}
        <div
          class="group/ep hover:bg-muted/50 grid cursor-pointer grid-cols-[28px_220px_1fr] items-start gap-5 p-4 transition-colors"
          role="button"
          tabindex="0"
          onclick={() => open(ep)}
          onkeydown={(e) => e.key === "Enter" && open(ep)}
          oncontextmenu={(e) => openMenu(ep, e)}
        >
          <div class="text-muted-foreground num pt-0.5 text-right text-sm">{ep.index ?? ""}</div>
          <div class="bg-muted relative aspect-video overflow-hidden rounded-lg ring-1 ring-foreground/10 [&_img]:size-full [&_img]:object-cover">
            <Img src={img(ep.thumb, 480, 270, ep.patinaServer)} class={spoiler ? "blur-[14px] brightness-75" : ""} />
            {#if p != null}
              <div class="absolute inset-x-0 bottom-0 h-[3px] bg-black/55"><i class="bg-tint block h-full" style:width="{p * 100}%"></i></div>
            {/if}
            <Button
              variant="secondary"
              size="icon-sm"
              class="absolute bottom-2.5 left-2 rounded-full bg-foreground pl-0.5 text-background opacity-0 transition-opacity group-hover/ep:opacity-100 hover:bg-foreground/90 focus-visible:opacity-100"
              title="Play"
              onclick={(e) => {
                e.stopPropagation();
                play(ep);
              }}><Play fill="currentColor" /></Button
            >
          </div>
          <div class="min-w-0">
            <h3 class="text-sm font-medium group-hover/ep:underline group-hover/ep:decoration-border group-hover/ep:underline-offset-4">{plex.title(ep)}</h3>
            <div class="text-muted-foreground mt-1 text-xs">
              {[ep.originallyAvailableAt && airDate(ep.originallyAvailableAt), ep.duration && duration(ep.duration)]
                .filter(Boolean)
                .join(" · ")}
              {#if plex.isWatched(ep)}<span class="text-tint">· Watched</span>{/if}
            </div>
            {#if ep.summary}
              <p class={["text-muted-foreground mt-2 line-clamp-3 max-w-[75ch] text-sm transition-[filter]", spoiler && "blur-[5px] hover:blur-none"]}>{ep.summary}</p>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {:else if kind === "album"}
    <div class="section-head"><h2>Tracks</h2></div>
    <div class="rows">
      {#each list as t (t.ratingKey)}
        <div
          class="list-row clickable grid-cols-[28px_1fr_auto]"
          role="button"
          tabindex="0"
          onclick={() => play(t)}
          onkeydown={(e) => e.key === "Enter" && play(t)}
          oncontextmenu={(e) => openMenu(t, e)}
        >
          <span class="text-muted-foreground num text-right text-sm">{t.index ?? ""}</span>
          <div class="grow">
            <div class="t">{plex.title(t)}</div>
            {#if t.originalTitle}<div class="s">{t.originalTitle}</div>{/if}
          </div>
          <div class="end text-sm">{t.duration ? clock(t.duration) : ""}</div>
        </div>
      {/each}
    </div>
  {:else if kind === "collection"}
    <div class="section-head">
      <h2>{items.length} item{items.length === 1 ? "" : "s"}</h2>
      {#if reorderable}<span class="text-muted-foreground text-xs">Drag to reorder</span>{/if}
    </div>
    <div class="poster-grid" bind:this={grid} onclickcapture={(e) => dropped && e.stopPropagation()}>
      {#each items as m (m.ratingKey)}
        {@const rk = m.ratingKey ?? ""}
        <div
          class={[
            "cell group/cell",
            {
              movable: reorderable,
              dragging: drag?.moved && drag.rk === rk,
              "drop-before": drag?.moved && before === rk,
              "drop-after": drag?.moved && before === null && rk === items[items.length - 1]?.ratingKey,
            },
          ]}
          role="presentation"
          data-rk={rk}
          onpointerdown={(e) => down(e, rk)}
          onpointermove={move}
          onpointerup={up}
          onpointercancel={up}
        >
          <Card item={m} />
          {#if editable}
            <Button
              variant="secondary"
              size="icon-xs"
              class="remove absolute top-1.5 right-1.5 bg-background/80 opacity-0 backdrop-blur-sm group-hover/cell:opacity-100 focus-visible:opacity-100"
              title="Remove from collection"
              onpointerdown={(e) => e.stopPropagation()}
              onclick={(e) => {
                e.stopPropagation();
                removeItem(m);
              }}><X /></Button
            >
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <div class="section-head"><h2>{kind === "artist" ? "Albums" : `${list.length} items`}</h2></div>
    <div class="poster-grid">
      {#each list as m (m.ratingKey)}<Card item={m} />{/each}
    </div>
  {/if}
{/if}

<style>
  .cell {
    position: relative;
    min-width: 0;
  }
  .cell.movable {
    touch-action: none;
  }
  .cell.dragging {
    opacity: 0.35;
  }
  .cell.drop-before::before,
  .cell.drop-after::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    border-radius: 1px;
    background: var(--tint);
  }
  .cell.drop-before::before {
    left: -10px;
  }
  .cell.drop-after::after {
    right: -10px;
  }
  .cell.dragging :global(.remove) {
    display: none;
  }
</style>
