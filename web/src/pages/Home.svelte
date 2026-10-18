<script lang="ts">
  import { ChevronLeft, ChevronRight, ChevronUp, ChevronDown, Play } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Label } from "$lib/components/ui/label";
  import HubRow from "../components/HubRow.svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { play } from "../lib/actions.svelte";
  import { api } from "../lib/api";
  import { duration } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav, routeFor } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";
  import type { HomeLayout, Hub, Metadata } from "../lib/types";

  const hubs = resource(() => {
    app.revision;
    return api.homeHubs();
  });

  let customizing = $state(false);
  const hubKey = (h: Hub) => h.hubIdentifier ?? h.key ?? h.title ?? "";
  /** Hubs hidden until turned on: Plex's legacy On Deck, which Continue Watching replaces. */
  const HIDDEN_BY_DEFAULT = ["home.ondeck"];
  const EMPTY: HomeLayout = { order: [], hidden: [], shown: [], compact_continue: false };
  let layout = $derived({ ...EMPTY, ...app.settings?.ui.home_layouts?.[app.scope] });
  const isHidden = (key: string) =>
    layout.hidden.includes(key) || (HIDDEN_BY_DEFAULT.includes(key) && !layout.shown.includes(key));
  function setShown(key: string, on: boolean) {
    const hidden = layout.hidden.filter((k) => k !== key), shown = layout.shown.filter((k) => k !== key);
    if (HIDDEN_BY_DEFAULT.includes(key)) saveLayout({ hidden, shown: on ? [...shown, key] : shown });
    else saveLayout({ hidden: on ? hidden : [...hidden, key], shown });
  }
  let allRows = $derived((hubs.data ?? []).filter((h) => h.Metadata.length || h.Directory.length).sort((a, b) => {
    const ai = layout.order.indexOf(hubKey(a)), bi = layout.order.indexOf(hubKey(b));
    return (ai < 0 ? 100000 : ai) - (bi < 0 ? 100000 : bi);
  }));
  let rows = $derived(allRows.filter((h) => !isHidden(hubKey(h))));
  function saveLayout(patch: Partial<typeof layout>) {
    app.updateUi({ home_layouts: { ...app.settings?.ui.home_layouts, [app.scope]: { ...layout, ...patch } } });
  }
  function move(index: number, delta: number) {
    const order = allRows.map(hubKey), target = index + delta;
    if (target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    saveLayout({ order });
  }

  /** Featured items: recent or in-progress movies and shows with backdrop art. */
  function featured(list: Hub[]) {
    const out: [Metadata, string][] = [];
    for (const hub of list) {
      const id = (hub.hubIdentifier ?? "").toLowerCase();
      const label = /continue|ondeck/.test(id) ? "Continue watching" : id.includes("recent") ? "Recently added" : null;
      if (!label) continue;
      for (const m of hub.Metadata) {
        const k = plex.kind(m);
        if (out.length < 6 && m.art && (k === "movie" || k === "show") && !out.some(([x]) => x.ratingKey === m.ratingKey))
          out.push([m, label]);
      }
    }
    return out;
  }

  let picks = $derived(featured(rows));
  let index = $state(0);
  let current = $derived(picks.length ? picks[index % picks.length] : null);
  const pad = (n: number) => String(n).padStart(2, "0");
</script>

<div class="flex justify-end gap-2 px-(--gutter) pt-4 pb-1">
  <Button variant="ghost" size="sm" onclick={() => nav.push({ name: "pick" })}>Pick something to watch</Button>
  <Button variant="ghost" size="sm" aria-expanded={customizing} onclick={() => (customizing = !customizing)}>Customize Home</Button>
</div>
{#if customizing}
  <div class="mx-(--gutter) mt-2 mb-6 overflow-hidden rounded-xl border bg-card">
    <div class="flex items-center gap-3 px-4 py-3">
      <Checkbox id="home-compact" checked={layout.compact_continue} onCheckedChange={(v) => saveLayout({ compact_continue: v === true })} />
      <Label for="home-compact" class="flex-1 cursor-pointer">Compact Continue Watching</Label>
    </div>
    {#each allRows as hub, index (hubKey(hub))}
      <div class="flex items-center gap-3 border-t px-4 py-2">
        <Checkbox id={`home-hub-${index}`} checked={!isHidden(hubKey(hub))} onCheckedChange={(v) => setShown(hubKey(hub), v === true)} />
        <Label for={`home-hub-${index}`} class="flex-1 cursor-pointer">{hub.title}</Label>
        <Button variant="ghost" size="icon-sm" title={`Move ${hub.title} up`} disabled={index === 0} onclick={() => move(index, -1)}><ChevronUp /></Button>
        <Button variant="ghost" size="icon-sm" title={`Move ${hub.title} down`} disabled={index === allRows.length - 1} onclick={() => move(index, 1)}><ChevronDown /></Button>
      </div>
    {/each}
    <div class="flex justify-end border-t bg-muted/30 px-4 py-2.5">
      <Button variant="ghost" size="sm" onclick={() => saveLayout(EMPTY)}>Reset layout</Button>
    </div>
  </div>
{/if}
{#if hubs.error && hubs.data !== undefined}<div class="px-(--gutter)"><Status error={hubs.error} retry={() => hubs.reload()} stale /></div>{/if}
{#if hubs.data === undefined}
  <div class="px-(--gutter)"><Status loading={!hubs.error} error={hubs.error} retry={() => hubs.reload()} fill /></div>
{:else if rows.length === 0}
  <div class="px-(--gutter)"><Status title="Nothing here yet" text={allRows.length ? "Your Home rows are hidden. Use Customize Home to show them." : "Your libraries are empty or still scanning."} fill /></div>
{:else}
  {#if current}
    {@const [item, label] = current}
    {@const meta = [
      item.year,
      item.contentRating,
      plex.kind(item) === "show"
        ? item.childCount != null && `${item.childCount} season${item.childCount === 1 ? "" : "s"}`
        : item.duration != null && duration(item.duration),
      item.Genre.slice(0, 2)
        .map((g) => g.tag)
        .join(", "),
    ].filter(Boolean)}
    <section class="hero relative mx-(--gutter) mt-2 h-[min(56vh,520px)] min-h-90 overflow-hidden rounded-xl border bg-card">
      {#key item.ratingKey}
        <div class="absolute inset-0"><Img src={img(item.art, 1920, 1080)} class="size-full object-cover object-[center_20%]" /></div>
      {/key}
      <div class="absolute inset-0 bg-linear-to-r from-background/95 via-background/55 to-transparent to-75%"></div>
      <div class="absolute inset-0 bg-linear-to-t from-background/70 to-transparent to-40%"></div>
      <div class="absolute bottom-9 left-9 z-10 max-w-xl">
        <Badge variant="secondary">{label}</Badge>
        <h1 class="mt-3 text-5xl leading-[1.05] font-semibold tracking-tight">{plex.title(item)}</h1>
        <div class="mt-3 text-sm text-muted-foreground">{meta.join("  ·  ")}</div>
        {#if item.summary}<p class="mt-3.5 line-clamp-3 text-sm leading-relaxed text-muted-foreground">{item.summary}</p>{/if}
        <div class="mt-5 flex gap-2">
          <Button size="lg" onclick={() => play(item)}>
            <Play fill="currentColor" />{plex.progress(item) != null ? "Resume" : "Play"}
          </Button>
          <Button size="lg" variant="secondary" onclick={() => nav.push(routeFor(item))}>Details</Button>
        </div>
      </div>
      {#if picks.length > 1}
        <div class="absolute right-6 bottom-8 z-10 flex items-center gap-1.5 text-xs text-muted-foreground tabular-nums">
          <Button variant="outline" size="icon-sm" title="Previous" onclick={() => (index = (index + picks.length - 1) % picks.length)}><ChevronLeft /></Button>
          <span class="px-1">{pad((index % picks.length) + 1)} / {pad(picks.length)}</span>
          <Button variant="outline" size="icon-sm" title="Next" onclick={() => (index = (index + 1) % picks.length)}><ChevronRight /></Button>
        </div>
      {/if}
    </section>
  {/if}
  <div class="page hubs" class:flush={!current}>
    {#each rows as hub, i (`${hub.hubIdentifier}:${hub.key}:${i}`)}
      <HubRow {hub} compact={layout.compact_continue} />
    {/each}
  </div>
{/if}

<style>
  .hubs {
    padding-top: 4px;
  }
  .hubs.flush {
    padding-top: 0;
  }
</style>
