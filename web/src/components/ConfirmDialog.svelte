<script lang="ts">
  // Confirmation for destructive actions, opened with `app.confirm(...)`.
  import { app } from "../lib/state.svelte";

  let c = $derived(app.confirming);

  function done(ok: boolean) {
    c?.resolve(ok);
    app.confirming = null;
  }
</script>

{#if c}
  <div class="modal-backdrop" role="presentation" onclick={() => done(false)}>
    <div
      class="modal"
      role="alertdialog"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        if (e.key === "Escape") done(false);
        if (e.key === "Enter") done(true);
      }}
    >
      <h2>{c.title}</h2>
      {#if c.text}<p class="desc">{c.text}</p>{/if}
      <div class="foot">
        <button class="btn quiet" onclick={() => done(false)}>Cancel</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class={["btn", c.danger ? "danger" : "primary"]} autofocus onclick={() => done(true)}>{c.action}</button>
      </div>
    </div>
  </div>
{/if}

