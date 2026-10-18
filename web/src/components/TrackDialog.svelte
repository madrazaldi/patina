<script lang="ts">
  // Set audio and subtitle tracks for many episodes at once. The matching is
  // done in Rust (crates/app/src/tracks.rs); this dialog only collects choices.
  import { TriangleAlert } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import * as Empty from "$lib/components/ui/empty";
  import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel } from "$lib/components/ui/field";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { Progress } from "$lib/components/ui/progress";
  import { Separator } from "$lib/components/ui/separator";
  import { Spinner } from "$lib/components/ui/spinner";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { app, captureScope, message } from "../lib/state.svelte";
  import type { Metadata, TrackView } from "../lib/types";

  let item = $derived(app.tracksTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();

  let loaded = $state(false);
  let error = $state<string>();
  let season = $state<number | null>(null);
  let reference = $state<string | null>(null);
  let audio = $state<number | null>(null);
  let subtitle = $state("keep");
  /** Also save the choices as this show's playback preferences. */
  let remember = $state(false);
  let view = $state<TrackView>();
  let progress = $state<[number, number] | null>(null);
  let busy = $derived(progress !== null);

  function showOf(m: Metadata): [string | undefined, string] {
    switch (plex.kind(m)) {
      case "show":
        return [m.ratingKey, plex.title(m)];
      case "season":
        return [m.parentRatingKey, m.parentTitle ?? ""];
      default:
        return [m.grandparentRatingKey, m.grandparentTitle ?? ""];
    }
  }

  let show = $derived(item ? showOf(item) : [undefined, ""]);

  // Load the show's episodes whenever the dialog opens.
  $effect(() => {
    const target = item;
    loaded = false;
    error = undefined;
    view = undefined;
    progress = null;
    if (!target) return;
    const k = plex.kind(target);
    season = k === "season" ? (target.index ?? null) : k === "episode" ? (target.parentIndex ?? null) : null;
    reference = k === "episode" ? (target.ratingKey ?? null) : null;
    audio = null;
    subtitle = "keep";
    const opened = (guard = captureScope());
    remember = false;
    const [rk] = showOf(target);
    if (!rk) {
      error = "This item doesn't belong to a show.";
      return;
    }
    api.tracksLoad(rk).then(
      () => app.tracksTarget === target && opened.valid() && (loaded = true),
      (e) => app.tracksTarget === target && opened.valid() && (error = message(e)),
    );
  });

  // Re-plan whenever a choice changes.
  $effect(() => {
    const rk = show[0];
    if (!loaded || !rk) return;
    const query = { show: rk, season, reference, audio, subtitle };
    const asked = guard;
    api.tracksView(query).then(
      (v) => asked.valid() && (view = v),
      (e) => asked.valid() && (error = message(e)),
    );
  });

  function close() {
    if (!busy) app.tracksTarget = null;
  }

  function resetTracks() {
    audio = null;
    subtitle = "keep";
  }

  let canApply = $derived(!!view && (view.plan.changes.length > 0 || (remember && !view.nothingChosen)));

  async function apply() {
    if (!view) return;
    if (!guard.valid()) { app.tracksTarget = null; return; }
    const changes = view.plan.changes;
    progress = [0, changes.length];
    try {
      // Before applying: applying drops the loaded tracks the rules are read from.
      if (remember && show[0]) await api.tracksRemember({ show: show[0], season, reference, audio, subtitle });
      if (changes.length) await api.tracksApply(changes, (done, total) => (progress = [done, total]));
    } catch (e) {
      app.error(e);
    }
    progress = null;
    app.tracksTarget = null;
  }

  const epCount = (n: number) => `${n} episode${n === 1 ? "" : "s"}`;
  const missHead = (n: number, what: string) => `${epCount(n)} ${n === 1 ? "has" : "have"} ${what}, left unchanged`;
</script>

{#if item}
  <dialog class="modal wide" use:modal={close} aria-labelledby="tracks-title">
    <h2 id="tracks-title">Audio & subtitles</h2>
    <p class="desc">{show[1]}: pick tracks in one episode and Patina selects the closest match in every other.</p>

    {#if error}
      <Alert.Root variant="destructive"><Alert.Description>{error}</Alert.Description></Alert.Root>
    {:else if !view}
      <div class="text-muted-foreground flex flex-col items-center gap-3 py-10 text-sm">
        <Spinner class="size-5" />
        <span>Reading tracks from every episode…</span>
      </div>
    {:else if view.total === 0}
      <Empty.Root class="border border-dashed p-6">
        <Empty.Header>
          <Empty.Title>No episodes</Empty.Title>
          <Empty.Description>This show has no episodes.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <FieldGroup class="grid grid-cols-2 gap-x-4 gap-y-4">
        <Field>
          <FieldLabel for="tracks-season">Apply to</FieldLabel>
          <NativeSelect.Root
            id="tracks-season"
            class="w-full"
            data-initial-focus
            disabled={busy}
            value={season === null ? "all" : String(season)}
            onchange={(e) => {
              const v = e.currentTarget.value;
              season = v === "all" ? null : Number(v);
              reference = null;
              resetTracks();
            }}
          >
            <NativeSelect.Option value="all">All seasons ({epCount(view.total)})</NativeSelect.Option>
            {#each view.seasons as s (s.index)}
              <NativeSelect.Option value={String(s.index)}>{s.title} ({epCount(s.count)})</NativeSelect.Option>
            {/each}
          </NativeSelect.Root>
        </Field>
        <Field>
          <FieldLabel for="tracks-reference">Take tracks from</FieldLabel>
          <NativeSelect.Root
            id="tracks-reference"
            class="w-full"
            disabled={busy}
            value={view.reference ?? ""}
            onchange={(e) => {
              reference = e.currentTarget.value;
              resetTracks();
            }}
          >
            {#each view.scope as ep (ep.ratingKey)}
              <NativeSelect.Option value={ep.ratingKey}>{ep.label}</NativeSelect.Option>
            {/each}
          </NativeSelect.Root>
        </Field>
        <Field>
          <FieldLabel for="tracks-audio">Audio</FieldLabel>
          <NativeSelect.Root
            id="tracks-audio"
            class="w-full"
            disabled={busy || view.audio.length === 0}
            value={audio === null ? "keep" : String(audio)}
            onchange={(e) => {
              const v = e.currentTarget.value;
              audio = v === "keep" ? null : Number(v);
            }}
          >
            <NativeSelect.Option value="keep">Don't change</NativeSelect.Option>
            {#each view.audio as s (s.id)}
              <NativeSelect.Option value={String(s.id)}>{s.label}</NativeSelect.Option>
            {/each}
          </NativeSelect.Root>
        </Field>
        <Field>
          <FieldLabel for="tracks-subtitle">Subtitles</FieldLabel>
          <NativeSelect.Root id="tracks-subtitle" class="w-full" disabled={busy} bind:value={subtitle}>
            <NativeSelect.Option value="keep">Don't change</NativeSelect.Option>
            <NativeSelect.Option value="off">None (turn off)</NativeSelect.Option>
            {#each view.subtitles as s (s.id)}
              <NativeSelect.Option value={String(s.id)}>{s.label}{s.external ? " · external" : ""}</NativeSelect.Option>
            {/each}
          </NativeSelect.Root>
        </Field>
      </FieldGroup>

      <Field orientation="horizontal" class="mt-4 items-start">
        <Checkbox id="tracks-remember" bind:checked={remember} disabled={busy || view.nothingChosen} />
        <FieldContent>
          <FieldLabel for="tracks-remember" class="font-normal">Remember for this show</FieldLabel>
          <FieldDescription>Use these languages for new episodes too, and whenever the show starts playing.</FieldDescription>
        </FieldContent>
      </Field>

      <Separator class="mt-5" />
      <div class="mt-4 flex flex-col gap-2.5 text-sm">
        {#if progress}
          <Progress value={progress[0]} max={Math.max(1, progress[1])} />
          <span class="tabular-nums">Updating {progress[0]} of {progress[1]}…</span>
        {:else if view.nothingChosen}
          <span class="text-muted-foreground">
            Choose an audio or subtitle track. Matches are found by language first, then title, codec and channels.
          </span>
        {:else if view.plan.episodes === 0 && !view.plan.noAudio.length && !view.plan.noSubtitle.length}
          <span>Every episode already uses these tracks.</span>
        {:else}
          <span>{view.plan.episodes} of {epCount(view.scope.length)} will change.</span>
        {/if}
        {#if !busy && view.plan.noAudio.length}
          <details class="text-muted-foreground">
            <summary class="flex cursor-pointer list-none items-center gap-2"><TriangleAlert class="size-3.5 text-amber-500" />{missHead(view.plan.noAudio.length, `no audio matching “${view.audioName}”`)}</summary>
            <ul class="text-muted-foreground mt-2 max-h-[140px] list-disc overflow-auto pl-5 text-xs">{#each view.plan.noAudio as e (e)}<li>{e}</li>{/each}</ul>
          </details>
        {/if}
        {#if !busy && view.plan.noSubtitle.length}
          <details class="text-muted-foreground">
            <summary class="flex cursor-pointer list-none items-center gap-2">
              <TriangleAlert class="size-3.5 text-amber-500" />{missHead(view.plan.noSubtitle.length, `no subtitles matching “${view.subtitleName}”`)}
            </summary>
            <ul class="text-muted-foreground mt-2 max-h-[140px] list-disc overflow-auto pl-5 text-xs">{#each view.plan.noSubtitle as e (e)}<li>{e}</li>{/each}</ul>
          </details>
        {/if}
      </div>
    {/if}

    <div class="foot">
      <Button variant="outline" disabled={busy} onclick={close}>Cancel</Button>
      <Button disabled={busy || !canApply} onclick={apply}>
        {view && view.plan.episodes > 0 ? `Apply to ${epCount(view.plan.episodes)}` : remember && canApply ? "Remember" : "Apply"}
      </Button>
    </div>
  </dialog>
{/if}
