<script lang="ts">
  // A movie or show as Seerr knows it: request it (or more seasons), follow its
  // requests, or open the copy on the Plex server once it is there.
  import { ExternalLink, Play, Plus } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import Img from "../components/Img.svelte";
  import SeerrRequestDialog from "../components/SeerrRequestDialog.svelte";
  import SeerrRequestRow from "../components/SeerrRequestRow.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { airDate, duration } from "../lib/format";
  import { img } from "../lib/images";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { availabilityLabel, requestable, requestLabel, seerrStatus } from "../lib/seerr";
  import { app } from "../lib/state.svelte";
  import type { SeerrMediaType, SeerrSeason, SeerrStatus } from "../lib/types";

  let { mediaType, tmdbId }: { mediaType: SeerrMediaType; tmdbId: number } = $props();

  const data = resource(() => api.seerrDetails(mediaType, tmdbId));
  let status = $state<SeerrStatus | null>(null);
  $effect(() => {
    if (app.seerr) seerrStatus(`${app.seerr.url}|${app.scope}`).then((s) => (status = s), () => {});
  });

  let asking = $state<{ is4k: boolean } | null>(null);

  function seasonLabel(s: SeerrSeason) {
    if (s.status === "available") return "Available";
    if (s.status === "partial") return "Partly available";
    if (s.status === "processing") return "Processing";
    return s.requested ? requestLabel(s.requested) : null;
  }
</script>

{#if data.error && data.data !== undefined}<Status error={data.error} retry={() => data.reload()} stale />{/if}
{#if data.data === undefined}
  <Status loading={!data.error} error={data.error} retry={() => data.reload()} fill />
{:else}
  {@const d = data.data}
  {@const label = availabilityLabel(d.status)}
  {@const label4k = availabilityLabel(d.status4k)}
  {@const can = status?.can}
  {@const meta = [
    d.rating ? `★ ${d.rating.toFixed(1)}` : null,
    d.year ? String(d.year) : null,
    d.runtime ? duration(d.runtime * 60000) : null,
    d.mediaType === "tv" ? `${d.seasons.length} season${d.seasons.length === 1 ? "" : "s"}` : null,
    ...d.genres.slice(0, 3),
  ].filter((m): m is string => !!m)}

  <div class="relative">
    <div class="backdrop pointer-events-none absolute inset-x-0 top-0 h-[520px] overflow-hidden opacity-50">
      {#if d.backdrop}<Img src={img(d.backdrop, 1920, 1080)} />{/if}
      <div class="to-background absolute inset-0 bg-linear-to-b from-background/10 via-background/60"></div>
    </div>

    <div class="page relative pt-32">
      <header class="flex items-end gap-9">
        <div class="bg-muted ring-border relative aspect-[2/3] w-[210px] shrink-0 overflow-hidden rounded-xl shadow-2xl ring-1 [&_img]:size-full [&_img]:object-cover">
          {#if d.poster}<Img src={img(d.poster, 500, 750)} />{/if}
        </div>
        <div class="min-w-0 flex-1 pb-1">
          <div class="text-muted-foreground mb-3 text-sm">{d.mediaType === "tv" ? "Show" : "Movie"} · Seerr</div>
          <h1 class="text-4xl leading-[1.08] font-semibold tracking-tight text-balance">{d.title}</h1>
          <div class="text-muted-foreground mt-3.5 flex flex-wrap items-center gap-x-2 gap-y-2 text-sm">
            {#each meta as m, i (i)}
              {#if i > 0}<span class="text-muted-foreground/50">·</span>{/if}<span>{m}</span>
            {/each}
            {#if label}<Badge variant="outline" class={d.status === "available" || d.status === "partial" ? "border-tint/40 bg-tint/10 text-tint" : ""}>{label}</Badge>{/if}
            {#if label4k}<Badge variant="outline">4K · {label4k}</Badge>{/if}
          </div>

          <div class="mt-6 flex flex-wrap items-center gap-2">
            {#if d.ratingKey}
              {@const rk = d.ratingKey}
              <Button size="lg" class="h-10 gap-2 px-5" onclick={() => nav.push({ name: "item", ratingKey: rk })}><Play fill="currentColor" />Open in Plex</Button>
            {/if}
            {#if can && requestable(d, can, false)}
              <Button size="lg" variant={d.ratingKey ? "outline" : "default"} class="h-10 gap-2 px-5" onclick={() => (asking = { is4k: false })}>
                <Plus />{d.mediaType === "tv" && d.seasons.some((s) => s.status || s.requested) ? "Request more" : "Request"}
              </Button>
            {/if}
            {#if can && requestable(d, can, true)}
              <Button size="lg" variant="outline" class="h-10 px-4" onclick={() => (asking = { is4k: true })}>Request in 4K</Button>
            {/if}
            <Button variant="outline" size="lg" class="h-10 px-4" onclick={() => api.seerrOpen(d.mediaType, d.tmdbId).catch((e) => app.error(e))}>
              <ExternalLink />Open in Seerr
            </Button>
          </div>
          {#if status && !(d.mediaType === "tv" ? status.can.requestTv : status.can.requestMovie)}
            <p class="text-muted-foreground mt-3 text-xs">Your Seerr account may not request {d.mediaType === "tv" ? "shows" : "movies"}.</p>
          {/if}
        </div>
      </header>

      <div class="mt-10 min-w-0 max-w-[72ch]">
        {#if d.tagline}<div class="text-muted-foreground mb-3 text-lg italic">{d.tagline}</div>{/if}
        {#if d.overview}<p class="text-[15px] leading-relaxed select-text">{d.overview}</p>{/if}
      </div>

      {#if d.seasons.length}
        <div class="section-head"><h2>Seasons</h2></div>
        <div class="rows">
          {#each d.seasons as s (s.number)}
            {@const sl = seasonLabel(s)}
            <div class="list-row grid-cols-[1fr_auto]">
              <div class="grow">
                <div class="t">{s.name}</div>
                <div class="s">{[s.episodes != null ? `${s.episodes} episodes` : null, s.airDate ? airDate(s.airDate) : null].filter(Boolean).join(" · ")}</div>
              </div>
              <div class="end">
                {#if sl}<Badge variant="outline" class={s.status === "available" ? "border-tint/40 bg-tint/10 text-tint" : ""}>{sl}</Badge>{/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      {#if d.requests.length}
        <div class="section-head"><h2>Requests</h2></div>
        <div class="rows">
          {#each d.requests as r (r.id)}
            <SeerrRequestRow request={r} can={can ?? null} linked={false} onchange={() => data.reload()} />
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if asking && can}
    <SeerrRequestDialog
      details={d}
      is4k={asking.is4k}
      {can}
      onclose={() => (asking = null)}
      ondone={() => {
        asking = null;
        data.reload();
      }}
    />
  {/if}
{/if}

<style>
  .backdrop :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
  }
</style>
