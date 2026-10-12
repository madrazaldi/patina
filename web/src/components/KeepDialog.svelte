<script lang="ts">
  // "Keep the next N unwatched episodes downloaded" for a show, season or
  // playlist. The policy is evaluated in crates/core/src/smart.rs.
  import { api } from "../lib/api";
  import { bytes } from "../lib/format";
  import * as plex from "../lib/plex";
  import { app, message } from "../lib/state.svelte";
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

  $effect(() => {
    const key = rk;
    loaded = false;
    existing = undefined;
    alsoDelete = false;
    if (!key) return;
    api.smartDownloads().then(
      (o) => {
        if (rk !== key) return;
        existing = o.policies.find((p) => p.active && p.ratingKey === key);
        keep = existing?.keep ?? 3;
        deleteWatched = existing?.deleteWatched ?? true;
        loaded = true;
      },
      (e) => {
        app.error(message(e));
        app.keepTarget = null;
      },
    );
  });

  const close = () => !busy && (app.keepTarget = null);

  async function run(work: () => Promise<unknown>, done: string) {
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

<svelte:window onkeydown={(e) => e.key === "Escape" && item && close()} />

{#if item && loaded}
  <div class="modal-backdrop" role="presentation" onclick={close}>
    <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>Keep downloaded</h2>
      <p class="desc">
        {plex.title(item)}: keep the next unwatched {noun} downloaded. New ones are fetched as you watch, and as they
        are added to the server.
      </p>

      <div class="rows">
        <div class="setting">
          <div class="label"><b>Keep next</b><span>Unwatched {noun} to keep on this computer.</span></div>
          <div class="control">
            <input class="input num" type="number" min="1" max="50" style="width: 80px" bind:value={keep} disabled={busy} />
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
    </div>
  </div>
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
