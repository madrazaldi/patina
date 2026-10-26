<script lang="ts">
  import { ExternalLink } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import * as Alert from "$lib/components/ui/alert";
  import * as Avatar from "$lib/components/ui/avatar";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import * as Field from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { Slider } from "$lib/components/ui/slider";
  import { Switch } from "$lib/components/ui/switch";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { initials } from "../lib/format";
  import { img } from "../lib/images";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { act, app, message } from "../lib/state.svelte";
  import PlaybackRulesForm from "../components/PlaybackRulesForm.svelte";
  import { forgetSeerrStatus, seerrStatus } from "../lib/seerr";
  import type { HomeUser, PlaybackPrefs, PlaybackRules, SeerrStatus, ShowRules } from "../lib/types";

  let p = $derived(app.settings?.player);
  let ui = $derived(app.settings?.ui);
  let info = $derived(app.info);
  let user = $derived(app.session?.user);

  let probe = $state<{ ok: boolean; text: string } | null>(null);
  function test() {
    probe = null;
    api.probeMpv().then(
      (text) => (probe = { ok: true, text }),
      (e) => (probe = { ok: false, text: message(e) }),
    );
  }
  test();

  // Automatic playback preferences (profile level); per-show ones are listed below them.
  let prefs = $state<PlaybackPrefs>();
  let shows = $state<ShowRules[]>([]);
  let prefsError = $state<string>();
  let rulesSet = $derived(prefs ? Object.values(prefs.profile).some((v) => v != null) : false);

  async function loadPrefs() {
    try {
      [prefs, shows] = await Promise.all([api.playbackPrefs(null), api.playbackPrefsShows()]);
      prefsError = undefined;
    } catch (e) {
      prefsError = message(e);
    }
  }
  // Runs on open, and again for another account or server (each has its own rules).
  $effect(() => {
    app.scope;
    loadPrefs();
  });

  async function saveProfile(rules: PlaybackRules) {
    try {
      await api.setPlaybackPrefs(null, null, rules);
      await loadPrefs();
    } catch (e) {
      app.error(e);
    }
  }

  async function clearShow(s: ShowRules) {
    if (await act(api.setPlaybackPrefs(s.ratingKey, s.title, {}), "Cleared")) loadPrefs();
  }

  /** "English audio · subtitles for foreign dialogue · 1080p" */
  function summary(r: PlaybackRules) {
    const bits: string[] = [];
    if (r.audioLanguages?.length) bits.push(`${r.audioLanguages.join(", ")} audio`);
    if (r.subtitles && r.subtitles !== "keep")
      bits.push(
        r.subtitles === "off"
          ? "no subtitles"
          : r.subtitles === "always"
            ? `subtitles${r.subtitleLanguages?.length ? ` (${r.subtitleLanguages.join(", ")})` : ""}`
            : "subtitles for foreign dialogue",
      );
    if (r.forced && r.forced !== "auto") bits.push(`${r.forced} forced`);
    if (r.avoidCommentary != null) bits.push(r.avoidCommentary ? "skip commentary" : "allow commentary");
    if (r.resolution && r.resolution !== "any") bits.push(r.resolution === "4k" ? "4K" : r.resolution === "sd" ? "SD" : `${r.resolution}p`);
    if (r.codec) bits.push(r.codec);
    if (r.hdr && r.hdr !== "any") bits.push(r.hdr === "prefer" ? "HDR" : "SDR");
    return bits.join(" · ") || "No rules";
  }

  // Seerr: the saved instance is checked on open; the form connects a new one (checked before saving).
  let seerr = $state<SeerrStatus | null>(null);
  let seerrError = $state<string>();
  let seerrUrl = $state("");
  let seerrKey = $state("");
  let connecting = $state(false);
  $effect(() => {
    const config = app.seerr;
    seerr = null;
    seerrError = undefined;
    if (!config) return;
    seerrUrl = config.url;
    seerrStatus(`${config.url}|${app.scope}`).then((s) => app.seerr?.url === config.url && (seerr = s), (e) => (seerrError = message(e)));
  });

  async function connectSeerr() {
    connecting = true;
    seerrError = undefined;
    try {
      // An empty field keeps a saved key; disconnect first to switch to the Plex sign-in.
      const status = await api.seerrConnect(seerrUrl, seerrKey.trim() === "" && app.seerr?.auth === "apiKey" ? null : seerrKey);
      forgetSeerrStatus();
      seerrKey = "";
      await app.loadSeerr();
      seerr = status;
      app.toast(`Connected to Seerr as ${status.user.name}`);
    } catch (e) {
      seerrError = message(e);
    }
    connecting = false;
  }

  async function disconnectSeerr() {
    if (await act(api.seerrDisconnect(), "Seerr disconnected")) {
      forgetSeerrStatus();
      seerrUrl = "";
      await app.loadSeerr();
    }
  }

  const home = resource(() => (app.hasTv ? api.homeUsers() : Promise.resolve([])));
  let pinFor = $state<HomeUser | null>(null);
  let pin = $state("");

  async function switchTo(u: HomeUser, code?: string) {
    pinFor = null;
    if (!u.uuid) return;
    nav.reset();
    await api.switchUser(u.uuid, code).catch((e) => app.error(e));
  }

  // Shared layout of the cards and their label / control rows.
  const CARD = "gap-0 pb-0";
  const HEAD = "border-b";
  const BODY = "divide-y px-0";
  const ROW = "flex items-center justify-between gap-6 px-4 py-3.5";
  const CTRL = "flex flex-none items-center gap-2";
</script>

{#snippet label(title: string, desc?: string, cls = "")}
  <Field.Content>
    <Field.Title>{title}</Field.Title>
    {#if desc}<Field.Description class={["text-[13px]", cls]}>{desc}</Field.Description>{/if}
  </Field.Content>
{/snippet}

{#if p && ui && info}
  <div class="page max-w-[960px]">
    <div class="page-head">
      <div>
        <h1>Settings</h1>
        <div class="sub selectable">Saved to {info.configPath}</div>
      </div>
    </div>

    <div class="flex flex-col gap-6">
      <Card.Root class={CARD}>
        <Card.Header class={HEAD}>
          <h2 class="text-base leading-snug font-semibold tracking-tight">Player</h2>
          <Card.Description>Everything plays in mpv, with your normal mpv configuration, scripts and key bindings.</Card.Description>
        </Card.Header>
        <Card.Content class={BODY}>
          <div class={ROW}>
            {#if !probe}{@render label("mpv executable", "Checking…")}
            {:else}{@render label("mpv executable", probe.text, probe.ok ? "" : "text-destructive")}{/if}
            <div class={CTRL}>
              <Input class="w-60" placeholder="Auto-detect" value={p.mpv_path} oninput={(e) => app.updatePlayer({ mpv_path: e.currentTarget.value })} />
              <Button variant="outline" size="sm" onclick={() => setTimeout(test, 300)}>Test</Button>
            </div>
          </div>
          <div class={ROW}>
            {@render label("mpv configuration", `${info.mpvConfigDir ?? ""}${info.mpvConfExists ? " (mpv.conf found)" : ""}`, "selectable")}
            <div class={CTRL}><Button variant="outline" size="sm" onclick={() => api.openMpvConfig().catch((e) => app.error(e))}>Open folder</Button></div>
          </div>
          <div class={ROW}>
            {@render label("Extra mpv arguments", "Appended to the command line, e.g. --fs --profile=hq. Your mpv.conf is always loaded.")}
            <div class={CTRL}>
              <Input class="w-72" placeholder="--fs" value={p.extra_args} oninput={(e) => app.updatePlayer({ extra_args: e.currentTarget.value })} />
            </div>
          </div>
          <div class={ROW}>
            {@render label("Streaming quality", "Original plays the file directly. Lower qualities ask the server to transcode, which helps on slow connections.")}
            <div class={CTRL}>
              <NativeSelect.Root class="w-56" value={p.quality} onchange={(e) => app.updatePlayer({ quality: e.currentTarget.value })}>
                {#each info.qualities as q (q.id)}<NativeSelect.Option value={q.id}>{q.label}</NativeSelect.Option>{/each}
              </NativeSelect.Root>
            </div>
          </div>
          {#each [
            ["continuous", "Autoplay next episode", "Queue the following episodes when you play one."],
            ["sync_streams", "Sync audio & subtitle tracks", "Start with the tracks selected in Plex, and remember tracks you switch to in mpv."],
            ["auto_skip_intro", "Skip intros automatically", "Uses Plex intro markers. Otherwise press the skip key when prompted."],
            ["auto_skip_credits", "Skip credits automatically", "Jumps past credits, or straight to the next episode."],
          ] as const as [key, title, desc] (key)}
            <div class={ROW}>
              {@render label(title, desc)}
              <div class={CTRL}>
                <Switch aria-label={title} checked={p[key]} onCheckedChange={(on) => app.updatePlayer({ [key]: on })} />
              </div>
            </div>
          {/each}
          <div class={ROW}>
            {@render label("Skip key", "mpv key name that skips an intro or credits. It is bound only while Patina controls mpv.")}
            <div class={CTRL}>
              <Input class="w-32" value={p.skip_key} oninput={(e) => app.updatePlayer({ skip_key: e.currentTarget.value })} />
            </div>
          </div>
          <div class={ROW}>
            {@render label("Mark as watched after", "How much of an item must play before it counts as watched.")}
            <div class={CTRL}>
              <NativeSelect.Root class="w-28" value={String(p.watched_threshold)} onchange={(e) => app.updatePlayer({ watched_threshold: Number(e.currentTarget.value) })}>
                {#each [0.8, 0.85, 0.9, 0.95] as t (t)}<NativeSelect.Option value={String(t)}>{Math.round(t * 100)}%</NativeSelect.Option>{/each}
              </NativeSelect.Root>
            </div>
          </div>
          <div class={ROW}>
            {@render label("System media controls", "Show what's playing in the OS media controls and respond to media keys.")}
            <div class={CTRL}>
              <Switch aria-label="System media controls" checked={p.media_controls} onCheckedChange={(on) => app.updatePlayer({ media_controls: on })} />
            </div>
          </div>
          <div class={ROW}>
            {@render label("Discord Rich Presence", "Show what you're watching or listening to on your Discord profile.")}
            <div class={CTRL}>
              <Switch aria-label="Discord Rich Presence" checked={p.discord} onCheckedChange={(on) => app.updatePlayer({ discord: on })} />
            </div>
          </div>
          {#if p.discord}
            <div class={ROW}>
              {@render label("Discord application ID", "Create an application at discord.com/developers and paste its ID here. Its name is what Discord shows, e.g. “Patina”.")}
              <div class={CTRL}>
                <Input
                  class="w-52 tabular-nums"
                  placeholder="Application ID"
                  value={p.discord_app_id}
                  oninput={(e) => app.updatePlayer({ discord_app_id: e.currentTarget.value.trim() })}
                />
                <Button variant="outline" size="sm" onclick={() => api.openUrl("https://discord.com/developers/applications").catch((e) => app.error(e))}>
                  <ExternalLink />Developer portal
                </Button>
              </div>
            </div>
          {/if}
          <div class={ROW}>
            {@render label("Downloads folder", "Where movies, episodes and music you download for offline viewing are saved.")}
            <div class={CTRL}>
              <Input
                class="w-60"
                placeholder="Default (Movies/Patina)"
                value={p.downloads_dir}
                oninput={(e) => app.updatePlayer({ downloads_dir: e.currentTarget.value })}
              />
              <Button variant="outline" size="sm" onclick={() => api.openDownloadsFolder().catch((e) => app.error(e))}>Open</Button>
            </div>
          </div>
        </Card.Content>
      </Card.Root>

      <Card.Root class={CARD}>
        <Card.Header class={HEAD}>
          <h2 class="text-base leading-snug font-semibold tracking-tight">Automatic playback</h2>
          <Card.Description>
            Audio, subtitle and version choices made for you when something starts playing, for the signed-in profile on
            this server. Shows can override these from their page. Applies to new episodes too.
          </Card.Description>
          {#if rulesSet}
            <Card.Action><Button variant="ghost" size="sm" onclick={() => saveProfile({})}>Clear all</Button></Card.Action>
          {/if}
        </Card.Header>
        <Card.Content class={BODY}>
          {#if prefsError}
            <div class="p-4">
              <Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap select-text">{prefsError}</Alert.Description></Alert.Root>
            </div>
          {:else if prefs}
            <PlaybackRulesForm class="rounded-none border-0 bg-transparent" rules={prefs.profile} onsave={saveProfile} />
            {#if shows.length}
              <h3 class="bg-muted/30 text-muted-foreground px-4 py-2 text-xs font-medium tracking-wider uppercase">Shows with their own preferences</h3>
              {#each shows as s (s.ratingKey)}
                <div class={ROW}>
                  {@render label(s.title, summary(s.rules))}
                  <div class={CTRL}>
                    <Button variant="outline" size="sm" onclick={() => nav.push({ name: "item", ratingKey: s.ratingKey })}>Open</Button>
                    <Button variant="destructive" size="sm" onclick={() => clearShow(s)}>Clear</Button>
                  </div>
                </div>
              {/each}
            {/if}
          {/if}
        </Card.Content>
      </Card.Root>

      <Card.Root class={CARD}>
        <Card.Header class={HEAD}>
          <h2 class="text-base leading-snug font-semibold tracking-tight">Appearance</h2>
        </Card.Header>
        <Card.Content class={BODY}>
          <div class={ROW}>
            {@render label("Accent", "Used sparingly: progress, watched marks and focus.")}
            <div class={CTRL}>
              <ToggleGroup.Root
                type="single"
                spacing={2}
                bind:value={() => ui.accent, (id) => id && app.updateUi({ accent: id })}
                aria-label="Accent"
              >
                {#each info.accents as a (a.id)}
                  <ToggleGroup.Item
                    value={a.id}
                    title={a.label}
                    aria-label={a.label}
                    style="background: {a.color}"
                    class="ring-offset-card data-[state=on]:ring-foreground size-5 min-w-5 rounded-full p-0 ring-offset-2 data-[state=on]:ring-2"
                  />
                {/each}
              </ToggleGroup.Root>
            </div>
          </div>
          <div class={ROW}>
            {@render label("Poster size", `${ui.poster_size}px`, "tabular-nums")}
            <div class={CTRL}>
              <Slider
                type="single"
                class="w-56"
                min={120}
                max={240}
                step={4}
                value={ui.poster_size}
                onValueChange={(v) => app.updateUi({ poster_size: v })}
                aria-label="Poster size"
              />
            </div>
          </div>
          <div class={ROW}>
            {@render label("Hide spoilers", "Blur thumbnails and summaries of episodes you haven't watched. Hover a summary to reveal it.")}
            <div class={CTRL}>
              <Switch aria-label="Hide spoilers" checked={ui.hide_spoilers} onCheckedChange={(on) => app.updateUi({ hide_spoilers: on })} />
            </div>
          </div>
        </Card.Content>
      </Card.Root>

      <Card.Root class={CARD}>
        <Card.Header class={HEAD}>
          <h2 class="text-base leading-snug font-semibold tracking-tight">Seerr</h2>
          <Card.Description>
            Request movies and shows the server doesn't have through Seerr, Overseerr or Jellyseerr. Leave the API key
            empty to sign in with your Plex account.
          </Card.Description>
          {#if app.seerr}
            <Card.Action><Button variant="ghost" size="sm" disabled={connecting} onclick={disconnectSeerr}>Disconnect</Button></Card.Action>
          {/if}
        </Card.Header>
        <Card.Content class={BODY}>
          {#if app.seerr}
            <div class={ROW}>
              {#if seerr}
                {@render label(
                  `Connected as ${seerr.user.name}`,
                  `${seerr.url}${seerr.version ? ` · Seerr ${seerr.version}` : ""} · ${seerr.auth === "plex" ? "signed in with Plex" : "API key"}${seerr.can.manage ? " · can manage requests" : ""}`,
                  "selectable",
                )}
              {:else if seerrError}
                {@render label("Not connected", seerrError, "text-destructive selectable")}
              {:else}
                {@render label(app.seerr.url, "Checking…")}
              {/if}
              <div class={CTRL}><Button variant="outline" size="sm" onclick={() => nav.push({ name: "requests" })}>Open requests</Button></div>
            </div>
          {/if}
          <div class={ROW}>
            {@render label("Server address", "For example http://192.168.1.10:5055")}
            <div class={CTRL}><Input class="w-72" placeholder="http://seerr.local:5055" aria-label="Seerr address" bind:value={seerrUrl} /></div>
          </div>
          <div class={ROW}>
            {@render label("API key", "In Seerr under Settings → General. Requests made with it count as the Seerr admin's.")}
            <div class={CTRL}>
              <Input class="w-72" type="password" placeholder={app.seerr?.auth === "apiKey" ? "Saved (enter to replace)" : "Optional"} aria-label="Seerr API key" bind:value={seerrKey} />
              <Button size="sm" disabled={connecting || !seerrUrl.trim()} onclick={connectSeerr}>{connecting ? "Connecting…" : app.seerr ? "Reconnect" : "Connect"}</Button>
            </div>
          </div>
          {#if seerrError && !app.seerr}
            <div class="p-4">
              <Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap select-text">{seerrError}</Alert.Description></Alert.Root>
            </div>
          {/if}
        </Card.Content>
      </Card.Root>

      <Card.Root class={CARD}>
        <Card.Header class={HEAD}>
          <h2 class="text-base leading-snug font-semibold tracking-tight">Account</h2>
        </Card.Header>
        <Card.Content class={BODY}>
          {#if user}
            <div class={ROW}>
              <div class="flex min-w-0 items-center gap-3">
                <Avatar.Root size="lg" class="overflow-hidden">
                  {#if user.thumb}<Img class="size-full object-cover" src={img(user.thumb, 96, 96)} />{:else}<Avatar.Fallback>{initials(user.name)}</Avatar.Fallback>{/if}
                </Avatar.Root>
                <Field.Content>
                  <Field.Title>{user.name}</Field.Title>
                  <Field.Description>{user.email ?? ""}{user.plexPass ? " · Plex Pass" : ""}</Field.Description>
                </Field.Content>
              </div>
              <div class={CTRL}><Button variant="destructive" size="sm" onclick={() => api.signOut()}>Sign out</Button></div>
            </div>
          {:else}
            <div class={ROW}>
              {@render label("Direct server connection", "Not signed in to plex.tv.")}
              <div class={CTRL}><Button variant="outline" size="sm" onclick={() => api.signOut()}>Sign in with Plex</Button></div>
            </div>
          {/if}
          {#if (home.data?.length ?? 0) > 1}
            <div class="flex flex-col gap-3 px-4 py-3.5">
              {@render label("Plex Home", "Switch to another member of your Plex Home.")}
              <div class="flex flex-wrap gap-2">
                {#each home.data ?? [] as u (u.uuid)}
                  {@const active = !!u.uuid && u.uuid === user?.uuid}
                  {@const name = u.title ?? u.username ?? "User"}
                  <Button
                    variant={active ? "default" : "outline"}
                    size="sm"
                    class="rounded-full pl-1"
                    onclick={() => {
                      if (active) return;
                      if (u.protected) {
                        pin = "";
                        pinFor = u;
                      } else switchTo(u);
                    }}
                  >
                    <Avatar.Root class="size-5 overflow-hidden">
                      {#if u.thumb}<Img class="size-full object-cover" src={img(u.thumb, 40, 40)} />{:else}<Avatar.Fallback class="text-[8px]">{initials(name)}</Avatar.Fallback>{/if}
                    </Avatar.Root>
                    {name}{u.protected ? " · PIN" : ""}
                  </Button>
                {/each}
              </div>
            </div>
          {/if}
          <div class={ROW}>
            {@render label("Version", `Patina ${info.version}`, "tabular-nums")}
          </div>
        </Card.Content>
      </Card.Root>
    </div>
  </div>

  {#if pinFor}
    {@const u = pinFor}
    <dialog class="modal" use:modal={() => (pinFor = null)} aria-labelledby="pin-title">
      <h2 id="pin-title">Enter PIN</h2>
      <p class="desc">{u.title ?? u.username}'s profile is protected.</p>
      <Input
        class="h-11 text-center font-mono text-xl tracking-[0.4em] md:text-xl"
        type="password"
        maxlength={4}
        aria-label="PIN"
        data-initial-focus
        bind:value={pin}
        onkeydown={(e) => e.key === "Enter" && switchTo(u, pin)}
      />
      <div class="foot">
        <Button variant="ghost" onclick={() => (pinFor = null)}>Cancel</Button>
        <Button onclick={() => switchTo(u, pin)}>Switch</Button>
      </div>
    </dialog>
  {/if}
{/if}
