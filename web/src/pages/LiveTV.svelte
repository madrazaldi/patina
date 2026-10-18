<script lang="ts">
  // Live TV from the server's DVR: what's on now and next per channel, and
  // scheduled recordings with their rules.
  import { CalendarClock, ChevronDown, CircleDot, Play, Radio, Repeat, Trash } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import { Progress } from "$lib/components/ui/progress";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import { resource } from "../lib/resource.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { LiveChannel, LiveProgram } from "../lib/types";

  let tab = $state<"guide" | "recordings">("guide");

  /** Unix seconds, advanced every minute so finished programs drop off. */
  let now = $state(Date.now() / 1000);
  /** Bumped every 15 minutes to refetch the guide. */
  let refetch = $state(0);
  $effect(() => {
    const a = setInterval(() => (now = Date.now() / 1000), 60_000);
    const b = setInterval(() => refetch++, 15 * 60_000);
    return () => (clearInterval(a), clearInterval(b));
  });

  const guide = resource(() => {
    refetch;
    return tab === "guide" ? api.livetvGuide(6) : Promise.resolve(null);
  });
  const recordings = resource(() => {
    app.revision;
    return tab === "recordings" ? api.recordings() : Promise.resolve(null);
  });

  /** Programs not yet over; the first is on air when it has begun. */
  const upcoming = (ch: LiveChannel) => ch.programs.filter((p) => p.endsAt > now);
  const onAir = (p: LiveProgram | undefined) => p != null && p.beginsAt <= now;
  const elapsed = (p: LiveProgram) => Math.min(100, Math.max(0, ((now - p.beginsAt) / (p.endsAt - p.beginsAt)) * 100));

  const time = (s: number) => new Date(s * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const range = (p: LiveProgram) => `${time(p.beginsAt)} – ${time(p.endsAt)}`;
  const when = (s: number) =>
    new Date(s * 1000).toLocaleString([], { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });

  /** Channel whose later listings are showing. */
  let open = $state<string | null>(null);
  const toggle = (id: string) => (open = open === id ? null : id);

  function watch(ch: LiveChannel) {
    app.toast("Tuning…");
    api.livetvWatch(ch.dvr, ch.id, ch.title).catch((e) => app.error(e));
  }

  const record = (guid: string) => act(api.record(guid), "Recording scheduled");

  async function deleteRule(id: string, title: string) {
    const ok = await app.confirm("Delete recording rule?", {
      text: `${title} will no longer be recorded. Existing recordings are kept.`,
      action: "Delete",
      danger: true,
    });
    if (ok && (await act(api.deleteRecordingRule(id), "Recording rule deleted"))) recordings.reload();
  }
</script>
{#if recordings.error && recordings.data}<Status error={recordings.error} retry={() => recordings.reload()} stale />{/if}
{#if guide.error && guide.data}<Status error={guide.error} retry={() => guide.reload()} stale />{/if}

{#snippet recordBtn(p: LiveProgram)}
  {@const guid = p.guid}
  {#if guid}
    <Button
      variant="ghost"
      size="icon-xs"
      class="hover:text-destructive opacity-0 group-hover/line:opacity-100 focus-visible:opacity-100"
      title="Record"
      onclick={(e) => {
        e.stopPropagation();
        record(guid);
      }}><CircleDot /></Button
    >
  {/if}
{/snippet}

<div class="page">
  <div class="page-head">
    <div>
      <h1>Live TV</h1>
      <div class="sub">Watch and record from your server's tuner.</div>
    </div>
    <div class="tabs">
      <button class={["tab", { active: tab === "guide" }]} onclick={() => (tab = "guide")}>Guide</button>
      <button class={["tab", { active: tab === "recordings" }]} onclick={() => (tab = "recordings")}>Recordings</button>
    </div>
  </div>

  {#if tab === "guide"}
    {@const channels = guide.data}
    {#if !channels}
      <Status loading={!guide.error} error={guide.error} retry={() => guide.reload()} />
    {:else if channels.length === 0}
      <Empty.Root class="border">
        <Empty.Header>
          <Empty.Media variant="icon"><Radio /></Empty.Media>
          <Empty.Title>No channels</Empty.Title>
          <Empty.Description>Set up a tuner and scan for channels in Plex's Live TV & DVR settings.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <div class="rows">
        {#each channels as ch (ch.dvr + ch.id)}
          {@const list = upcoming(ch)}
          {@const cur = onAir(list[0]) ? list[0] : undefined}
          {@const next = cur ? list[1] : list[0]}
          {@const later = list.slice(cur ? 2 : 1)}
          {@const expanded = open === ch.dvr + ch.id}
          <div>
            <div
              class="group/line hover:bg-muted/40 grid min-h-16 cursor-pointer grid-cols-[200px_minmax(0,1.4fr)_minmax(0,1fr)_auto] items-center gap-5 px-4 py-2.5 transition-colors"
              role="button"
              tabindex="0"
              onclick={() => toggle(ch.dvr + ch.id)}
              onkeydown={(e) => e.key === "Enter" && toggle(ch.dvr + ch.id)}
            >
              <div class="flex min-w-0 items-center gap-2.5 font-medium">
                <span class="text-muted-foreground w-8.5 flex-none text-right text-xs font-normal tabular-nums">{ch.number ?? ""}</span>
                <span class="bg-muted size-8 flex-none overflow-hidden rounded-md [&_img]:size-full [&_img]:object-contain"><Img src={img(ch.thumb, 80, 80)} /></span>
                <span class="ellipsis">{ch.title}</span>
              </div>

              <div class="min-w-0">
                {#if cur}
                  <div class="row-flex min-h-6 gap-1.5">
                    <span class="ellipsis font-medium">{cur.title}</span>
                    {@render recordBtn(cur)}
                  </div>
                  <div class="text-muted-foreground ellipsis text-xs">
                    <span class="tabular-nums">{range(cur)}</span>{#if cur.subtitle}{" "}· {cur.subtitle}{/if}
                  </div>
                  <Progress value={elapsed(cur)} class="mt-2 max-w-70 [&_[data-slot=progress-indicator]]:bg-tint" />
                {:else}
                  <span class="text-muted-foreground">{list.length ? "Off air" : "No guide data"}</span>
                {/if}
              </div>

              <div class="text-muted-foreground min-w-0">
                {#if next}
                  <div class="row-flex min-h-6 gap-1.5">
                    <span class="flex-none text-[11px] tabular-nums">{time(next.beginsAt)}</span>
                    <span class="ellipsis">{next.title}</span>
                    {@render recordBtn(next)}
                  </div>
                  {#if next.subtitle}<div class="ellipsis text-xs">{next.subtitle}</div>{/if}
                {/if}
              </div>

              <div class="flex items-center gap-2.5">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={(e) => {
                    e.stopPropagation();
                    watch(ch);
                  }}><Play />Watch</Button
                >
                <span class={["text-muted-foreground inline-flex transition-transform", { "rotate-180": expanded }]}><ChevronDown size={14} /></span>
              </div>
            </div>

            {#if expanded}
              <div class="flex gap-px overflow-x-auto pr-4 pb-3.5 pl-56">
                {#each later as p (p.beginsAt)}
                  <div class="flex w-48 flex-none flex-col gap-0.5 border-l px-3 py-2">
                    <div class="text-muted-foreground text-[11px] tabular-nums">{range(p)}</div>
                    <div class="ellipsis">{p.title}</div>
                    <div class="text-muted-foreground ellipsis text-xs">{p.subtitle ?? ""}</div>
                    {#if p.guid}
                      {@const guid = p.guid}
                      <Button variant="outline" size="xs" class="mt-1 self-start" onclick={() => record(guid)}>Record</Button>
                    {/if}
                  </div>
                {:else}
                  <span class="text-muted-foreground text-xs">No later listings in the next six hours.</span>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    {@const r = recordings.data}
    {#if !r}
      <Status loading={!recordings.error} error={recordings.error} retry={() => recordings.reload()} />
    {:else}
      <div class="section-head"><h2>Upcoming</h2></div>
      {#if r.upcoming.length === 0}
        <Empty.Root class="border">
          <Empty.Header>
            <Empty.Media variant="icon"><CalendarClock /></Empty.Media>
            <Empty.Description>Nothing scheduled. Record a program from the guide.</Empty.Description>
          </Empty.Header>
        </Empty.Root>
      {:else}
        <div class="rows">
          {#each r.upcoming as u (u.id)}
            <div class="list-row grid-cols-[170px_1fr_auto]">
              <span class="text-muted-foreground text-sm tabular-nums">{u.beginsAt ? when(u.beginsAt) : ""}</span>
              <div class="grow">
                <div class="t">{u.title}</div>
                <div class="s">{[u.subtitle, u.channel].filter(Boolean).join(" · ")}</div>
              </div>
              <div class="end">{#if u.status}<Badge variant="outline">{u.status}</Badge>{/if}</div>
            </div>
          {/each}
        </div>
      {/if}

      <div class="section-head"><h2>Rules</h2></div>
      {#if r.rules.length === 0}
        <Empty.Root class="border">
          <Empty.Header>
            <Empty.Media variant="icon"><Repeat /></Empty.Media>
            <Empty.Description>No recording rules.</Empty.Description>
          </Empty.Header>
        </Empty.Root>
      {:else}
        <div class="rows">
          {#each r.rules as rule (rule.id)}
            <div class="list-row grid-cols-[1fr_auto]">
              <div class="grow">
                <div class="t">{rule.title}</div>
                {#if rule.kind}<div class="s">{rule.kind}</div>{/if}
              </div>
              <div class="end">
                <Button variant="ghost" size="icon-sm" title="Delete rule" onclick={() => deleteRule(rule.id, rule.title)}>
                  <Trash />
                </Button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}

  <p class="text-muted-foreground mt-7 text-xs">Live TV needs a tuner connected to a Plex Pass server.</p>
</div>
