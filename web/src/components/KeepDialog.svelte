<script lang="ts">
  // "Keep the next N unwatched episodes downloaded" for a show, season or
  // playlist. The policy is evaluated in crates/core/src/smart.rs.
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
      <div class="setting">
        <div class="label"><b>Keep next</b><span>Unwatched {noun} to keep on this computer.</span></div>
        <div class="control">
          <input class="input num" data-initial-focus type="number" min="1" max="50" style="width: 80px" bind:value={keep} disabled={busy} />
        </div>
      </div>
      <div class="setting">
        <div class="label"><b>Remove after watching</b><span>Delete these downloads once they are watched.</span></div>
        <div class="control">
          <button class={["toggle", { on: deleteWatched }]} aria-label="Remove after watching" onclick={() => (deleteWatched = !deleteWatched)}></button>
        </div>
      </div>
      {#if existing}
        <div class="setting">
          <div class="label">
            <b>Currently</b>
            <span>{existing.downloads} downloaded{existing.bytes ? `, ${bytes(existing.bytes)}` : ""}{existing.enabled ? "" : " (paused)"}</span>
          </div>
          <div class="control">
            <label class="inline"><input type="checkbox" bind:checked={alsoDelete} />Also delete them</label>
          </div>
        </div>
      {/if}
    </div>

    <div class="foot">
      {#if existing}
        <button class="btn quiet danger" disabled={busy} onclick={remove}>Stop keeping</button>
      {/if}
      <button class="btn quiet" disabled={busy} onclick={close}>Cancel</button>
      <button class="btn primary" disabled={busy || !(keep >= 1 && keep <= 50)} onclick={save}>{existing ? "Save" : "Keep downloaded"}</button>
    </div>
  </dialog>
{/if}

<style>
  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
  }
</style>
