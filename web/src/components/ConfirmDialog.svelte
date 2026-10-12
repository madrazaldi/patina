<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { modal } from "../lib/dialog";
  let c = $derived(app.confirming);
  function done(ok: boolean) { c?.resolve(ok); app.confirming = null; }
</script>

{#if c}
  <dialog class="modal" role="alertdialog" use:modal={() => done(false)} aria-labelledby="confirm-title" aria-describedby={c.text ? "confirm-description" : undefined}>
    <h2 id="confirm-title">{c.title}</h2>
    {#if c.text}<p id="confirm-description" class="desc">{c.text}</p>{/if}
    <div class="foot">
      <button class="btn quiet" data-initial-focus={c.danger ? "" : undefined} onclick={() => done(false)}>Cancel</button>
      <button class={["btn", c.danger ? "danger" : "primary"]} data-initial-focus={!c.danger ? "" : undefined} onclick={() => done(true)}>{c.action}</button>
    </div>
  </dialog>
{/if}
