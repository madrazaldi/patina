<script lang="ts">
  import { setLucideProps } from "@lucide/svelte";
  import Shell from "./components/Shell.svelte";
  import Toasts from "./components/Toasts.svelte";
  import { api } from "./lib/api";
  import { play } from "./lib/actions.svelte";
  import { nav, parseRoute, routeKey } from "./lib/router.svelte";
  import { app } from "./lib/state.svelte";
  import Activity from "./pages/Activity.svelte";
  import Browse from "./pages/Browse.svelte";
  import Gate from "./pages/Gate.svelte";
  import Home from "./pages/Home.svelte";
  import Item from "./pages/Item.svelte";
  import Library from "./pages/Library.svelte";
  import Playlist from "./pages/Playlist.svelte";
  import Playlists from "./pages/Playlists.svelte";
  import Search from "./pages/Search.svelte";
  import Server from "./pages/Server.svelte";
  import Settings from "./pages/Settings.svelte";
  import Watchlist from "./pages/Watchlist.svelte";

  setLucideProps({ size: 16, strokeWidth: 1.5 });

  let ready = $state(false);
  app.init().then(
    () => (ready = true),
    (e) => {
      ready = true;
      app.stage = { kind: "failed", message: String(e) };
    },
  );

  let accent = $derived(app.info?.accents.find((a) => a.id === app.settings?.ui.accent)?.color);
  $effect(() => {
    const root = document.documentElement.style;
    if (accent) root.setProperty("--accent", accent);
    root.setProperty("--poster", `${app.settings?.ui.poster_size ?? 160}px`);
  });

  // Debug builds: PATINA_ROUTE=/item/100 opens a page, PATINA_PLAY=<ratingKey> starts playback.
  let debugDone = false;
  $effect(() => {
    if (app.stage.kind !== "ready" || debugDone || !app.info) return;
    debugDone = true;
    const route = app.info.debugRoute && parseRoute(app.info.debugRoute);
    if (route) nav.push(route);
    const rk = app.info.debugPlay;
    if (rk) api.metadata(rk).then((m) => m && play(m));
  });
</script>

{#if ready}
  {#if app.stage.kind === "ready" && app.session}
    {@const route = nav.route}
    <Shell>
      {#key routeKey(route)}
        <div class="fade-in">
          {#if route.name === "home"}
            <Home />
          {:else if route.name === "library"}
            <Library id={route.id} />
          {:else if route.name === "item"}
            <Item ratingKey={route.ratingKey} />
          {:else if route.name === "browse"}
            <Browse path={route.path} title={route.title} />
          {:else if route.name === "search"}
            <Search />
          {:else if route.name === "playlists"}
            <Playlists />
          {:else if route.name === "playlist"}
            <Playlist id={route.id} />
          {:else if route.name === "watchlist"}
            <Watchlist />
          {:else if route.name === "activity"}
            <Activity />
          {:else if route.name === "server"}
            <Server />
          {:else if route.name === "settings"}
            <Settings />
          {/if}
        </div>
      {/key}
    </Shell>
  {:else}
    <Gate />
  {/if}
{/if}
<Toasts />
