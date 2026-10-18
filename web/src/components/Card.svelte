<script lang="ts">
  // Poster / thumbnail card for any metadata item.
  import { Check, Ellipsis, Play } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { cn } from "$lib/utils";
  import Img from "./Img.svelte";
  import { openMenu, play } from "../lib/actions.svelte";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { nav, routeFor } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let {
    item,
    selecting = false, selected = false, ontoggle,
    shape: forced,
    useArt = false,
    fixed = false,
  }: {
    item: Metadata;
    selecting?: boolean; selected?: boolean; ontoggle?: () => void;
    shape?: plex.Shape;
    /** Continue-watching style: backdrop art, show name as the title. */
    useArt?: boolean;
    /** Fixed width (inside horizontal rows) rather than filling a grid cell. */
    fixed?: boolean;
  } = $props();

  const SIZES = { poster: [300, 450], landscape: [560, 315], square: [360, 360] } as const;

  let shape = $derived(forced ?? plex.shapeFor(item));
  let kind = $derived(plex.kind(item));
  let src = $derived.by(() => {
    const [w, h] = SIZES[shape];
    let source: string | undefined;
    if (shape === "landscape" && useArt && kind !== "episode") source = plex.backdrop(item) ?? item.thumb;
    else if (shape === "landscape") source = item.thumb ?? plex.backdrop(item);
    else source = plex.poster(item);
    return img(source, w, h, item.patinaServer);
  });
  let lines = $derived(
    kind === "episode" && useArt
      ? [item.grandparentTitle ?? "", `${plex.episodeCode(item)} · ${plex.title(item)}`]
      : [plex.title(item), plex.subtitle(item)],
  );
  let unwatched = $derived(
    kind === "show" || kind === "season" ? (plex.unwatchedLeaves(item) ?? 0) : 0,
  );
  let watched = $derived(plex.hasWatchState(item) && unwatched === 0 && plex.isWatched(item));
  let progress = $derived(plex.progress(item));
  let spoiler = $derived(app.spoilers && kind === "episode" && !plex.isWatched(item));

  const open = () => selecting ? ontoggle?.() : nav.push(routeFor(item));
</script>

<div
  class={cn(
    "card group/card min-w-0 cursor-pointer outline-none",
    shape,
    fixed && ["fixed-width flex-none", shape === "landscape" ? "w-[calc(var(--poster)*1.75)]" : "w-(--poster)"],
  )}
  role="button"
  aria-label={plex.fullTitle(item)}
  aria-pressed={selecting ? selected : undefined}
  tabindex="0"
  onclick={open}
  onkeydown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); open(); } }}
  oncontextmenu={(e) => openMenu(item, e)}
>
  <div
    class={cn(
      "art relative overflow-hidden rounded-lg bg-card ring-1 ring-foreground/10 transition-shadow group-hover/card:ring-foreground/25 group-focus-visible/card:ring-2 group-focus-visible/card:ring-tint",
      shape === "landscape" ? "aspect-video" : shape === "square" ? "aspect-square" : "aspect-[2/3]",
      selecting && selected && "ring-2 ring-tint",
    )}
  >
    {#if src}
      <Img
        {src}
        class={cn(
          "size-full object-cover group-hover/card:brightness-75",
          spoiler && "blur-[14px] brightness-70 group-hover/card:brightness-55",
        )}
      />
    {:else}
      <div class="flex h-full items-center justify-center p-3 text-center text-sm leading-tight text-muted-foreground">{plex.title(item)}</div>
    {/if}
    {#if unwatched > 0}
      <Badge variant="secondary" class="absolute top-2 right-2 bg-background/80 tabular-nums backdrop-blur-sm">{unwatched}</Badge>
    {:else if watched}
      <span class="absolute top-2 right-2 grid size-5 place-items-center rounded-full bg-background/80 text-tint backdrop-blur-sm" title="Watched"><Check size={12} strokeWidth={2.4} /></span>
    {/if}
    {#if progress != null}
      <div class="absolute inset-x-0 bottom-0 h-1 bg-black/55"><i class="block h-full bg-tint" style:width="{progress * 100}%"></i></div>
    {/if}
    {#if selecting}
      <span
        class={cn(
          "absolute top-2 left-2 grid size-5 place-items-center rounded-full border backdrop-blur-sm",
          selected ? "border-tint bg-tint text-background" : "border-foreground/40 bg-background/70",
        )}
      >{#if selected}<Check size={12} strokeWidth={3} />{/if}</span>
    {/if}
    <div
      class={cn(
        "absolute inset-0 bg-linear-to-t from-black/50 via-transparent to-transparent opacity-0 transition-opacity duration-150 group-hover/card:opacity-100 group-focus-within/card:opacity-100",
        selecting && "hidden",
      )}
    >
      {#if plex.canPlay(item)}
        <Button
          size="icon"
          class="absolute bottom-3 left-2 size-9 rounded-full shadow-md"
          title="Play"
          onclick={(e) => {
            e.stopPropagation();
            play(item);
          }}><Play fill="currentColor" class="ml-0.5" /></Button
        >
      {/if}
      <Button
        variant="ghost"
        size="icon-sm"
        class="absolute top-2 left-2 bg-background/70 backdrop-blur-sm hover:bg-background/90"
        title="More"
        onclick={(e) => openMenu(item, e)}><Ellipsis /></Button
      >
    </div>
  </div>
  <div class="mt-2 truncate text-sm leading-5 font-medium" title={lines[0]}>{lines[0]}</div>
  {#if lines[1]}<div class="truncate text-xs leading-4 text-muted-foreground">{lines[1]}</div>{/if}
</div>
