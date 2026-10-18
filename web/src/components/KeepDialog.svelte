<script lang="ts">
  // "Keep the next N unwatched episodes downloaded" for a show, season or
  // playlist. The policy is evaluated in crates/core/src/smart.rs.
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Field, FieldContent, FieldDescription, FieldLabel } from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Switch } from "$lib/components/ui/switch";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { bytes } from "../lib/format";
  import * as plex from "../lib/plex";
  import { app, captureScope, message } from "../lib/state.svelte";
  import type { SmartPolicyView } from "../lib/types";

  let item = $derived(app.keepTarget);
  let rk = $derived(item?.ratingKey);
  let noun = $derived(item && plex.kind(item) === "playlist" ? "items" : "episodes");

  let existing = $state<SmartPolicyView>();
  let loaded = $state(false);
  let keep = $state(3);
  let deleteWatched = $state(true);
  let alsoDelete = $state(false);
  let busy = $state(false);
  let guard = captureScope();

  $effect(() => {
    const key = rk;
    loaded = false;
    existing = undefined;
    alsoDelete = false;
    if (!key) return;
    const opened = (guard = captureScope());
    api.smartDownloads().then(
      (o) => {
        if (rk !== key || !opened.valid()) return;
        existing = o.policies.find((p) => p.active && p.ratingKey === key);
        keep = existing?.keep ?? 3;
        deleteWatched = existing?.deleteWatched ?? true;
        loaded = true;
      },
      (e) => {
        if (!opened.valid()) return;
        app.error(message(e));
        app.keepTarget = null;
      },
    );
  });

  const close = () => !busy && (app.keepTarget = null);

  async function run(work: () => Promise<unknown>, done: string) {
    if (!guard.valid()) {
      app.keepTarget = null;
      return;
    }
    busy = true;
    try {
      await work();
      app.toast(done);
      app.keepTarget = null;
    } catch (e) {
      app.error(e);
    }
    busy = false;
  }

  const save = () => run(() => api.setSmartDownload(rk ?? "", keep, deleteWatched), "Smart download saved");
  const remove = () =>
    existing && run(() => api.removeSmartDownload(existing!.id, alsoDelete), alsoDelete ? "Policy and downloads removed" : "Policy removed");
</script>

{#if item && loaded}
  <dialog class="modal" use:modal={close} aria-labelledby="keep-title">
    <h2 id="keep-title">Keep downloaded</h2>
    <p class="desc">
      {plex.title(item)}: keep the next unwatched {noun} downloaded. New ones are fetched as you watch, and as they
      are added to the server.
    </p>

    <div class="rows">
      <Field orientation="horizontal" class="justify-between gap-6 px-4 py-3.5">
        <FieldContent>
          <FieldLabel for="keep-count">Keep next</FieldLabel>
          <FieldDescription>Unwatched {noun} to keep on this computer.</FieldDescription>
        </FieldContent>
        <Input id="keep-count" class="w-20 tabular-nums" data-initial-focus type="number" min="1" max="50" bind:value={keep} disabled={busy} />
      </Field>
      <Field orientation="horizontal" class="justify-between gap-6 px-4 py-3.5">
        <FieldContent>
          <FieldLabel for="keep-delete-watched">Remove after watching</FieldLabel>
          <FieldDescription>Delete these downloads once they are watched.</FieldDescription>
        </FieldContent>
        <Switch id="keep-delete-watched" aria-label="Remove after watching" bind:checked={deleteWatched} disabled={busy} />
      </Field>
      {#if existing}
        <Field orientation="horizontal" class="justify-between gap-6 px-4 py-3.5">
          <FieldContent>
            <FieldLabel>Currently</FieldLabel>
            <FieldDescription>{existing.downloads} downloaded{existing.bytes ? `, ${bytes(existing.bytes)}` : ""}{existing.enabled ? "" : " (paused)"}</FieldDescription>
          </FieldContent>
          <div class="flex items-center gap-2">
            <Checkbox id="keep-also-delete" bind:checked={alsoDelete} disabled={busy} />
            <Label for="keep-also-delete" class="text-muted-foreground text-xs font-normal">Also delete them</Label>
          </div>
        </Field>
      {/if}
    </div>

    <div class="foot">
      {#if existing}
        <Button variant="destructive" class="mr-auto" disabled={busy} onclick={remove}>Stop keeping</Button>
      {/if}
      <Button variant="outline" disabled={busy} onclick={close}>Cancel</Button>
      <Button disabled={busy || !(keep >= 1 && keep <= 50)} onclick={save}>{existing ? "Save" : "Keep downloaded"}</Button>
    </div>
  </dialog>
{/if}
