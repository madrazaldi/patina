<script lang="ts">
  // Everything before the library: splash, sign-in, server choice, failure —
  // and, when the server can't be reached, playback of downloads offline.
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import NowPlaying from "../components/NowPlaying.svelte";
  import { api } from "../lib/api";
  import { app, message } from "../lib/state.svelte";
  import Downloads from "./Downloads.svelte";

  type Mode =
    | { kind: "choose" }
    | { kind: "browser"; url: string }
    | { kind: "code"; code: string }
    | { kind: "password" }
    | { kind: "manual" };

  let mode = $state<Mode>({ kind: "choose" });
  let busy = $state(false);
  let error = $state<string>();

  let login = $state("");
  let password = $state("");
  let code = $state("");
  let url = $state("http://localhost:32400");
  let token = $state("");

  async function startPin(strong: boolean) {
    error = undefined;
    busy = true;
    try {
      const pin = await api.startPin(strong);
      busy = false;
      mode = strong ? { kind: "browser", url: pin.url } : { kind: "code", code: pin.code };
      // Resolves once signed in (the stage changes) or when cancelled.
      await api.completePin(pin.id, pin.attempt);
    } catch (e) {
      error = message(e);
      mode = { kind: "choose" };
    }
    busy = false;
  }

  function cancel() {
    api.cancelPin();
    mode = { kind: "choose" };
  }

  async function submit(f: () => Promise<void>) {
    busy = true;
    error = undefined;
    try {
      await f();
    } catch (e) {
      error = message(e);
    }
    busy = false;
  }

  /** Showing downloads instead of the failure screen. */
  let offline = $state(false);
  let canOffline = $derived(app.downloads.some((d) => d.state === "done"));
  const retry = () => api.retry().catch((e) => app.error(e));

  const back = () => {
    error = undefined;
    mode = { kind: "choose" };
  };
</script>

{#if offline && (app.stage.kind === "failed" || app.stage.kind === "connecting")}
  <div class="offline">
    <header class="top" data-tauri-drag-region>
      <span class="serif name">Patina</span>
      <span class="faint">{app.stage.kind === "connecting" ? `Connecting to ${app.stage.name}…` : "Offline"}</span>
      <span class="grow"></span>
      {#if app.stage.kind === "connecting"}
        <div class="spinner small"></div>
      {:else}
        <button class="btn sm" onclick={retry}>Try again</button>
      {/if}
    </header>
    <div class="scroll"><Downloads offline /></div>
    <NowPlaying />
  </div>
  <ConfirmDialog />
{:else}
  <div class="gate" data-tauri-drag-region>
    <div class="column">
      <div class="brand serif">Patina</div>

      {#if app.stage.kind === "booting" || app.stage.kind === "ready"}
        <div class="spinner"></div>
      {:else if app.stage.kind === "connecting"}
        <div class="spinner"></div>
        <p class="muted">Connecting to {app.stage.name}…</p>
      {:else if app.stage.kind === "failed"}
        <div class="error-box">{app.stage.message}</div>
        <div class="stack">
          <button class="btn primary" onclick={retry}>Try again</button>
          {#if canOffline}<button class="btn" onclick={() => (offline = true)}>Play downloads offline</button>{/if}
          <button class="btn" onclick={() => api.signOut()}>Sign out</button>
        </div>
      {:else if app.stage.kind === "pickServer"}
        <p class="muted">Signed in as {app.stage.user.name}. Choose a server.</p>
        <div class="rows servers">
          {#each app.stage.servers as s (s.id)}
            <button class="list-row clickable" onclick={() => api.pickServer(s.id)}>
              <span class="grow">
                <span class="t">{s.name}</span>
                <span class="s">
                  {s.owned ? "Owned" : `Shared by ${s.sourceTitle ?? "someone"}`}
                  {#if s.platform} · {s.platform}{/if}
                  {#if s.version} · v{s.version}{/if}
                </span>
              </span>
              <span class={["tag", { accent: s.presence }]}>{s.presence ? "Online" : "Offline?"}</span>
            </button>
          {/each}
        </div>
        <button class="link" onclick={() => api.signOut()}>Use another account</button>
      {:else if mode.kind === "choose"}
        <p class="muted">A Plex client that plays everything in mpv, with your own mpv configuration.</p>
        <div class="stack">
          <button class="btn primary lg" disabled={busy} onclick={() => startPin(true)}>Sign in with Plex</button>
          <button class="btn lg" disabled={busy} onclick={() => startPin(false)}>Use a link code</button>
        </div>
        <div class="alt">
          <button class="link" onclick={() => (mode = { kind: "password" })}>Email and password</button>
          <button class="link" onclick={() => (mode = { kind: "manual" })}>Connect to a server directly</button>
        </div>
      {:else if mode.kind === "browser"}
        {@const link = mode.url}
        <div class="spinner"></div>
        <p class="muted">Approve Patina in the browser window that just opened.</p>
        <div class="alt">
          <button class="link" onclick={() => api.openUrl(link)}>Open it again</button>
          <button class="link" onclick={cancel}>Cancel</button>
        </div>
      {:else if mode.kind === "code"}
        <p class="muted">
          Go to <button class="link" onclick={() => api.openUrl("https://plex.tv/link")}>plex.tv/link</button> on any device and
          enter
        </p>
        <div class="pin num">{mode.code}</div>
        <p class="faint">Waiting for approval…</p>
        <button class="link" onclick={cancel}>Cancel</button>
      {:else if mode.kind === "password"}
        <form class="stack" onsubmit={(e) => (e.preventDefault(), submit(() => api.passwordSignIn(login, password, code)))}>
          <input class="input" placeholder="Email or username" bind:value={login} />
          <input class="input" type="password" placeholder="Password" bind:value={password} />
          <input class="input" placeholder="Two-factor code (if enabled)" bind:value={code} />
          <button class="btn primary" disabled={busy || !login || !password}>Sign in</button>
        </form>
        <button class="link" onclick={back}>Back</button>
      {:else if mode.kind === "manual"}
        <p class="faint small">
          Enter the server address and an X-Plex-Token. Watchlist and Plex Home need a plex.tv sign-in.
        </p>
        <form class="stack" onsubmit={(e) => (e.preventDefault(), submit(() => api.signInManual(url, token)))}>
          <input class="input" placeholder="http://192.168.1.10:32400" bind:value={url} />
          <input class="input" placeholder="X-Plex-Token" bind:value={token} />
          <button class="btn primary" disabled={busy || !token.trim()}>Connect</button>
        </form>
        <button class="link" onclick={back}>Back</button>
      {/if}

      {#if error}<div class="error-box">{error}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .gate {
    display: grid;
    place-items: center;
    height: 100vh;
    padding: 40px;
  }
  .column {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
    width: 340px;
    text-align: center;
  }
  .brand {
    font-size: 36px;
    letter-spacing: -0.015em;
    margin-bottom: 6px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .alt {
    display: flex;
    gap: 18px;
    font-size: 12.5px;
  }
  .small {
    font-size: 12.5px;
  }
  .pin {
    font: 400 40px/1 var(--mono);
    letter-spacing: 0.2em;
    padding: 6px 0 4px;
    user-select: text;
    -webkit-user-select: text;
  }
  .servers {
    width: 100%;
    text-align: left;
  }
  .servers button {
    grid-template-columns: 1fr auto;
    width: 100%;
    border-left: 0;
    border-right: 0;
    background: none;
  }
  .servers .t,
  .servers .s {
    display: block;
  }
  .error-box {
    width: 100%;
    text-align: left;
  }

  .offline {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 48px;
    flex: none;
    /* Clear the window buttons of the overlay title bar. */
    padding: 0 calc(var(--gutter) - 8px) 0 84px;
    border-bottom: 1px solid var(--line);
  }
  .name {
    font-size: 17px;
  }
  .spinner.small {
    width: 14px;
    height: 14px;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
</style>
