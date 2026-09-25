<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { modal } from "../lib/dialog";
  let c = $derived(app.confirming);
  function done(ok: boolean) { c?.resolve(ok); app.confirming = null; }
</script>

{#if c}
  <dialog class="modal" use:modal aria-labelledby="confirm-title" aria-describedby={c.text ? "confirm-description" : undefined}
    oncancel={(e) => { e.preventDefault(); done(false); }}
    onclick={(e) => { if (e.target === e.currentTarget) { const r = e.currentTarget.getBoundingClientRect(); if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) done(false); } }}>
    <h2 id="confirm-title">{c.title}</h2>
    {#if c.text}<p id="confirm-description" class="desc">{c.text}</p>{/if}
    <div class="foot">
      <button class="btn quiet" data-initial-focus={c.danger ? "" : undefined} onclick={() => done(false)}>Cancel</button>
      <button class={["btn", c.danger ? "danger" : "primary"]} data-initial-focus={!c.danger ? "" : undefined} onclick={() => done(true)}>{c.action}</button>
    </div>
  </dialog>
{/if}
<style>
  dialog { color: var(--text); margin: auto; }
  dialog::backdrop { background: rgba(0, 0, 0, .65); }
</style>
