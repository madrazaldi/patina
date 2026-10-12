<script lang="ts">
  // Automatic audio, subtitle and version preferences for one show. Fields left
  // unset inherit the profile's rules (Settings). Applied whenever the show
  // plays, including episodes added later.
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { app, message } from "../lib/state.svelte";
  import type { Metadata, PlaybackPrefs, PlaybackRules } from "../lib/types";
  import PlaybackRulesForm from "./PlaybackRulesForm.svelte";

  let item = $derived(app.prefsTarget);

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
  let prefs = $state<PlaybackPrefs>();
  let error = $state<string>();

  $effect(() => {
    const [rk] = show;
    prefs = undefined;
    error = undefined;
    if (!item) return;
    if (!rk) {
      error = "This item doesn't belong to a show.";
      return;
    }
    api.playbackPrefs(rk).then(
      (p) => show[0] === rk && (prefs = p),
      (e) => (error = message(e)),
    );
  });

  async function save(rules: PlaybackRules) {
    const [rk, title] = show;
    if (!rk) return;
    try {
      await api.setPlaybackPrefs(rk, title ?? null, rules);
      prefs = await api.playbackPrefs(rk);
    } catch (e) {
      app.error(e);
    }
  }

  const close = () => (app.prefsTarget = null);
  let own = $derived(prefs?.show);
  let empty = $derived(!own || Object.values(own).every((v) => v == null));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && item && close()} />

{#if item}
  <div class="modal-backdrop" role="presentation" onclick={close}>
    <div class="modal wide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>Playback preferences</h2>
      <p class="desc">
        {show[1]}: tracks and version picked automatically when this show plays, including new episodes.
        Anything left unset follows your profile's preferences in Settings.
      </p>

      {#if error}
        <div class="error-box">{error}</div>
      {:else if !prefs || !own}
        <div class="loading"><div class="spinner"></div></div>
      {:else}
        <PlaybackRulesForm rules={own} unset="Same as profile" onsave={save} />
      {/if}

      <div class="foot">
        <button class="btn quiet" disabled={empty} onclick={() => save({})}>Clear for this show</button>
        <button class="btn primary" onclick={close}>Done</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .loading {
    display: grid;
    place-items: center;
    padding: 40px 0;
  }
</style>
