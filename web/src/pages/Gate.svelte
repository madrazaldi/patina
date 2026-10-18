<script lang="ts">
  // Everything before the library: splash, sign-in, server choice, failure —
  // and, when the server can't be reached, playback of downloads offline.
  import * as Alert from "$lib/components/ui/alert";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import { Input } from "$lib/components/ui/input";
  import * as Item from "$lib/components/ui/item";
  import { Spinner } from "$lib/components/ui/spinner";
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

{#snippet linkButton(text: string, onclick: () => void)}
  <Button variant="link" size="sm" class="text-muted-foreground hover:text-foreground h-auto p-0" {onclick}>{text}</Button>
{/snippet}

{#if offline && (app.stage.kind === "failed" || app.stage.kind === "connecting")}
  <div class="flex h-screen flex-col">
    <!-- Clear the window buttons of the overlay title bar. -->
    <header class="flex h-12 flex-none items-center gap-3 border-b pr-[calc(var(--gutter)-8px)] pl-21" data-tauri-drag-region>
      <span class="text-[17px] font-semibold tracking-tight">Patina</span>
      <span class="text-muted-foreground text-sm">{app.stage.kind === "connecting" ? `Connecting to ${app.stage.name}…` : "Offline"}</span>
      <span class="flex-1"></span>
      {#if app.stage.kind === "connecting"}
        <Spinner class="text-muted-foreground" />
      {:else}
        <Button variant="outline" size="sm" onclick={retry}>Try again</Button>
      {/if}
    </header>
    <div class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto"><Downloads offline /></div>
    <NowPlaying />
  </div>
  <ConfirmDialog />
{:else}
  <div class="grid h-screen place-items-center p-10" data-tauri-drag-region>
    <div class="flex w-full max-w-sm flex-col items-center gap-6">
      <div class="text-4xl font-semibold tracking-tight">Patina</div>

      <Card.Root class="w-full">
        <Card.Content class="flex flex-col items-center gap-4 text-center">
          {#if app.stage.kind === "booting" || app.stage.kind === "ready"}
            <Spinner class="text-muted-foreground size-5" />
          {:else if app.stage.kind === "connecting"}
            <Spinner class="text-muted-foreground size-5" />
            <p class="text-muted-foreground text-sm">Connecting to {app.stage.name}…</p>
          {:else if app.stage.kind === "failed"}
            <Alert.Root variant="destructive"><Alert.Description class="select-text">{app.stage.message}</Alert.Description></Alert.Root>
            <div class="flex w-full flex-col gap-2">
              <Button onclick={retry}>Try again</Button>
              {#if canOffline}<Button variant="outline" onclick={() => (offline = true)}>Play downloads offline</Button>{/if}
              <Button variant="outline" onclick={() => api.signOut()}>Sign out</Button>
            </div>
          {:else if app.stage.kind === "pickServer"}
            <p class="text-muted-foreground text-sm">Signed in as {app.stage.user.name}. Choose a server.</p>
            <div class="flex w-full flex-col gap-2 text-left">
              {#each app.stage.servers as s (s.id)}
                <Item.Root variant="outline" size="sm" class="hover:bg-muted/50 cursor-pointer flex-nowrap">
                  {#snippet child({ props })}
                    <button {...props} type="button" onclick={() => api.pickServer(s.id)}>
                      <Item.Content class="min-w-0">
                        <Item.Title>{s.name}</Item.Title>
                        <Item.Description class="line-clamp-1">
                          {s.owned ? "Owned" : `Shared by ${s.sourceTitle ?? "someone"}`}
                          {#if s.platform} · {s.platform}{/if}
                          {#if s.version} · v{s.version}{/if}
                        </Item.Description>
                      </Item.Content>
                      <Badge variant="outline" class={s.presence ? "text-tint border-tint/40 bg-tint/10" : ""}>{s.presence ? "Online" : "Offline?"}</Badge>
                    </button>
                  {/snippet}
                </Item.Root>
              {/each}
            </div>
            {@render linkButton("Use another account", () => api.signOut())}
          {:else if mode.kind === "choose"}
            <p class="text-muted-foreground text-sm">A Plex client that plays everything in mpv, with your own mpv configuration.</p>
            <div class="flex w-full flex-col gap-2">
              <Button size="lg" disabled={busy} onclick={() => startPin(true)}>Sign in with Plex</Button>
              <Button variant="outline" size="lg" disabled={busy} onclick={() => startPin(false)}>Use a link code</Button>
            </div>
            <div class="flex gap-4">
              {@render linkButton("Email and password", () => (mode = { kind: "password" }))}
              {@render linkButton("Connect to a server directly", () => (mode = { kind: "manual" }))}
            </div>
          {:else if mode.kind === "browser"}
            {@const link = mode.url}
            <Spinner class="text-muted-foreground size-5" />
            <p class="text-muted-foreground text-sm">Approve Patina in the browser window that just opened.</p>
            <div class="flex gap-4">
              {@render linkButton("Open it again", () => api.openUrl(link))}
              {@render linkButton("Cancel", cancel)}
            </div>
          {:else if mode.kind === "code"}
            <p class="text-muted-foreground text-sm">
              Go to <Button variant="link" size="sm" class="h-auto p-0" onclick={() => api.openUrl("https://plex.tv/link")}>plex.tv/link</Button> on any device and
              enter
            </p>
            <div class="bg-muted/50 w-full rounded-lg border py-3 font-mono text-4xl leading-none tracking-[0.2em] tabular-nums select-text">{mode.code}</div>
            <p class="text-muted-foreground text-sm">Waiting for approval…</p>
            {@render linkButton("Cancel", cancel)}
          {:else if mode.kind === "password"}
            <form class="flex w-full flex-col gap-2" onsubmit={(e) => (e.preventDefault(), submit(() => api.passwordSignIn(login, password, code)))}>
              <Input placeholder="Email or username" bind:value={login} />
              <Input type="password" placeholder="Password" bind:value={password} />
              <Input placeholder="Two-factor code (if enabled)" bind:value={code} />
              <Button type="submit" disabled={busy || !login || !password}>Sign in</Button>
            </form>
            {@render linkButton("Back", back)}
          {:else if mode.kind === "manual"}
            <p class="text-muted-foreground text-[13px]">
              Enter the server address and an X-Plex-Token. Watchlist and Plex Home need a plex.tv sign-in.
            </p>
            <form class="flex w-full flex-col gap-2" onsubmit={(e) => (e.preventDefault(), submit(() => api.signInManual(url, token)))}>
              <Input placeholder="http://192.168.1.10:32400" bind:value={url} />
              <Input placeholder="X-Plex-Token" bind:value={token} />
              <Button type="submit" disabled={busy || !token.trim()}>Connect</Button>
            </form>
            {@render linkButton("Back", back)}
          {/if}

          {#if error}
            <Alert.Root variant="destructive"><Alert.Description class="select-text whitespace-pre-wrap">{error}</Alert.Description></Alert.Root>
          {/if}
        </Card.Content>
      </Card.Root>
    </div>
  </div>
{/if}
