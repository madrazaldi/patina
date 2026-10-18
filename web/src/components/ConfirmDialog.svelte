<script lang="ts">
  import { Button } from "$lib/components/ui/button";
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
      <Button variant="outline" data-initial-focus={c.danger ? "" : undefined} onclick={() => done(false)}>Cancel</Button>
      <Button variant={c.danger ? "destructive" : "default"} data-initial-focus={!c.danger ? "" : undefined} onclick={() => done(true)}>{c.action}</Button>
    </div>
  </dialog>
{/if}
