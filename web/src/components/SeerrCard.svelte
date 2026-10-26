<script lang="ts">
  // Poster card for a Seerr title (search, discover). Opens its Seerr page.
  import { Check } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { cn } from "$lib/utils";
  import Img from "./Img.svelte";
  import { img } from "../lib/images";
  import { nav } from "../lib/router.svelte";
  import { availabilityLabel } from "../lib/seerr";
  import type { SeerrTitle } from "../lib/types";

  let { title: t }: { title: SeerrTitle } = $props();

  let label = $derived(availabilityLabel(t.status));
  let kind = $derived(t.mediaType === "tv" ? "Show" : "Movie");
  const open = () => nav.push({ name: "seerr", mediaType: t.mediaType, tmdbId: t.tmdbId });
</script>

<div
  class="card group/card min-w-0 cursor-pointer outline-none"
  role="button"
  tabindex="0"
  aria-label={`${t.title}${t.year ? ` (${t.year})` : ""}, ${kind}${label ? `, ${label}` : ""}`}
  onclick={open}
  onkeydown={(e) => {
    if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      open();
    }
  }}
>
  <div
    class="art relative aspect-[2/3] overflow-hidden rounded-lg bg-card ring-1 ring-foreground/10 transition-shadow group-hover/card:ring-foreground/25 group-focus-visible/card:ring-2 group-focus-visible/card:ring-tint"
  >
    {#if t.poster}
      <Img src={img(t.poster, 300, 450)} class="size-full object-cover group-hover/card:brightness-75" />
    {:else}
      <div class="flex h-full items-center justify-center p-3 text-center text-sm leading-tight text-muted-foreground">{t.title}</div>
    {/if}
    {#if t.status === "available"}
      <span class="absolute top-2 right-2 grid size-5 place-items-center rounded-full bg-background/80 text-tint backdrop-blur-sm" title="Available">
        <Check size={12} strokeWidth={2.4} />
      </span>
    {:else if label}
      <Badge variant="secondary" class={cn("absolute top-2 right-2 bg-background/80 backdrop-blur-sm", t.status === "partial" && "text-tint")}>{label}</Badge>
    {/if}
  </div>
  <div class="mt-2 truncate text-sm leading-5 font-medium" title={t.title}>{t.title}</div>
  <div class="truncate text-xs leading-4 text-muted-foreground">{[t.year, kind].filter(Boolean).join(" · ")}</div>
</div>
