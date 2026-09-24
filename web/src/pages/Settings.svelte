<script lang="ts">
  import Img from "../components/Img.svelte";
  import { api } from "../lib/api";
  import { initials } from "../lib/format";
  import { img } from "../lib/images";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { app, message } from "../lib/state.svelte";
  import type { HomeUser } from "../lib/types";

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

  const home = resource(() => (app.hasTv ? api.homeUsers() : Promise.resolve([])));
  let pinFor = $state<HomeUser | null>(null);
  let pin = $state("");

  async function switchTo(u: HomeUser, code?: string) {
    pinFor = null;
    if (!u.uuid) return;
    nav.reset();
    await api.switchUser(u.uuid, code).catch((e) => app.error(e));
  }
</script>

{#if p && ui && info}
  <div class="page narrow">
    <div class="page-head">
      <div>
        <h1>Settings</h1>
        <div class="sub selectable">Saved to {info.configPath}</div>
      </div>
    </div>

    <section>
      <div class="section-head"><h2>Player</h2></div>
      <p class="muted intro">Everything plays in mpv, with your normal mpv configuration, scripts and key bindings.</p>
      <div class="rows">
        <div class="setting">
          <div class="label">
            <b>mpv executable</b>
            {#if !probe}<span>Checking…</span>
            {:else}<span class={probe.ok ? "ok" : "bad"}>{probe.text}</span>{/if}
          </div>
          <div class="control">
            <input class="input" style="width: 240px" placeholder="Auto-detect" value={p.mpv_path} oninput={(e) => app.updatePlayer({ mpv_path: e.currentTarget.value })} />
            <button class="btn sm" onclick={() => setTimeout(test, 300)}>Test</button>
          </div>
        </div>
        <div class="setting">
          <div class="label">
            <b>mpv configuration</b>
            <span class="selectable">{info.mpvConfigDir}{info.mpvConfExists ? " (mpv.conf found)" : ""}</span>
          </div>
          <div class="control"><button class="btn sm" onclick={() => api.openMpvConfig().catch((e) => app.error(e))}>Open folder</button></div>
        </div>
        <div class="setting">
          <div class="label"><b>Extra mpv arguments</b><span>Appended to the command line, e.g. --fs --profile=hq. Your mpv.conf is always loaded.</span></div>
          <div class="control">
            <input class="input" style="width: 300px" placeholder="--fs" value={p.extra_args} oninput={(e) => app.updatePlayer({ extra_args: e.currentTarget.value })} />
          </div>
        </div>
        <div class="setting">
          <div class="label"><b>Streaming quality</b><span>Original plays the file directly. Lower qualities ask the server to transcode, which helps on slow connections.</span></div>
          <div class="control">
            <select class="select" style="width: 220px" value={p.quality} onchange={(e) => app.updatePlayer({ quality: e.currentTarget.value })}>
              {#each info.qualities as q (q.id)}<option value={q.id}>{q.label}</option>{/each}
            </select>
          </div>
        </div>
        {#each [
          ["continuous", "Autoplay next episode", "Queue the following episodes when you play one."],
          ["sync_streams", "Sync audio & subtitle tracks", "Start with the tracks selected in Plex, and remember tracks you switch to in mpv."],
          ["auto_skip_intro", "Skip intros automatically", "Uses Plex intro markers. Otherwise press the skip key when prompted."],
          ["auto_skip_credits", "Skip credits automatically", "Jumps past credits, or straight to the next episode."],
        ] as const as [key, label, desc] (key)}
          <div class="setting">
            <div class="label"><b>{label}</b><span>{desc}</span></div>
            <div class="control">
              <button class={["toggle", { on: p[key] }]} aria-label={label} onclick={() => app.updatePlayer({ [key]: !p[key] })}></button>
            </div>
          </div>
        {/each}
        <div class="setting">
          <div class="label"><b>Skip key</b><span>mpv key name that skips an intro or credits. It is bound only while Patina controls mpv.</span></div>
          <div class="control">
            <input class="input" style="width: 120px" value={p.skip_key} oninput={(e) => app.updatePlayer({ skip_key: e.currentTarget.value })} />
          </div>
        </div>
        <div class="setting">
          <div class="label"><b>Mark as watched after</b><span>How much of an item must play before it counts as watched.</span></div>
          <div class="control">
            <select
              class="select"
              style="width: 120px"
              value={String(p.watched_threshold)}
              onchange={(e) => app.updatePlayer({ watched_threshold: Number(e.currentTarget.value) })}
            >
              {#each [0.8, 0.85, 0.9, 0.95] as t (t)}<option value={String(t)}>{Math.round(t * 100)}%</option>{/each}
            </select>
          </div>
        </div>
      </div>
    </section>

    <section>
      <div class="section-head"><h2>Appearance</h2></div>
      <div class="rows">
        <div class="setting">
          <div class="label"><b>Accent</b><span>Used sparingly: progress, watched marks and focus.</span></div>
          <div class="control swatches">
            {#each info.accents as a (a.id)}
              <button
                class={["swatch", { on: ui.accent === a.id }]}
                title={a.label}
                aria-label={a.label}
                style:--c={a.color}
                onclick={() => app.updateUi({ accent: a.id })}
              ></button>
            {/each}
          </div>
        </div>
        <div class="setting">
          <div class="label"><b>Poster size</b><span class="num">{ui.poster_size}px</span></div>
          <div class="control">
            <input
              type="range"
              min="120"
              max="240"
              step="4"
              value={ui.poster_size}
              oninput={(e) => app.updateUi({ poster_size: Number(e.currentTarget.value) })}
            />
          </div>
        </div>
        <div class="setting">
          <div class="label"><b>Hide spoilers</b><span>Blur thumbnails and summaries of episodes you haven't watched. Hover a summary to reveal it.</span></div>
          <div class="control">
            <button class={["toggle", { on: ui.hide_spoilers }]} aria-label="Hide spoilers" onclick={() => app.updateUi({ hide_spoilers: !ui.hide_spoilers })}></button>
          </div>
        </div>
      </div>
    </section>

    <section>
      <div class="section-head"><h2>Account</h2></div>
      <div class="rows">
        {#if user}
          <div class="setting">
            <div class="label row-flex">
              <span class="avatar big">{#if user.thumb}<Img src={img(user.thumb, 96, 96)} />{:else}{initials(user.name)}{/if}</span>
              <div>
                <b>{user.name}</b>
                <span>{user.email ?? ""}{user.plexPass ? " · Plex Pass" : ""}</span>
              </div>
            </div>
            <div class="control"><button class="btn sm danger" onclick={() => api.signOut()}>Sign out</button></div>
          </div>
        {:else}
          <div class="setting">
            <div class="label"><b>Direct server connection</b><span>Not signed in to plex.tv.</span></div>
            <div class="control"><button class="btn sm" onclick={() => api.signOut()}>Sign in with Plex</button></div>
          </div>
        {/if}
        {#if (home.data?.length ?? 0) > 1}
          <div class="setting column">
            <div class="label"><b>Plex Home</b><span>Switch to another member of your Plex Home.</span></div>
            <div class="users">
              {#each home.data ?? [] as u (u.uuid)}
                {@const active = !!u.uuid && u.uuid === user?.uuid}
                {@const name = u.title ?? u.username ?? "User"}
                <button
                  class={["chip", { active }]}
                  onclick={() => {
                    if (active) return;
                    if (u.protected) {
                      pin = "";
                      pinFor = u;
                    } else switchTo(u);
                  }}
                >
                  <span class="avatar tiny">{#if u.thumb}<Img src={img(u.thumb, 40, 40)} />{:else}{initials(name)}{/if}</span>
                  {name}{u.protected ? " · PIN" : ""}
                </button>
              {/each}
            </div>
          </div>
        {/if}
        <div class="setting">
          <div class="label"><b>Version</b><span class="num">Patina {info.version}</span></div>
        </div>
      </div>
    </section>
  </div>

  {#if pinFor}
    {@const u = pinFor}
    <div class="modal-backdrop" role="presentation" onclick={() => (pinFor = null)}>
      <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
        <h2>Enter PIN</h2>
        <p class="desc">{u.title ?? u.username}'s profile is protected.</p>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="input pin"
          type="password"
          maxlength="4"
          autofocus
          bind:value={pin}
          onkeydown={(e) => e.key === "Enter" && switchTo(u, pin)}
        />
        <div class="foot">
          <button class="btn quiet" onclick={() => (pinFor = null)}>Cancel</button>
          <button class="btn primary" onclick={() => switchTo(u, pin)}>Switch</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .narrow {
    max-width: 960px;
  }
  .intro {
    margin-bottom: 8px;
  }
  .ok {
    color: var(--text-2) !important;
  }
  .bad {
    color: var(--danger) !important;
  }
  .swatches {
    gap: 10px;
  }
  .swatch {
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--c);
    cursor: pointer;
  }
  .swatch.on {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 3px var(--text);
  }
  input[type="range"] {
    width: 220px;
    accent-color: var(--accent);
  }
  .avatar.big {
    width: 40px;
    height: 40px;
    margin-right: 6px;
  }
  .avatar.tiny {
    width: 18px;
    height: 18px;
    font-size: 8px;
  }
  .setting.column {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }
  .users {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pin {
    width: 100%;
    font: 20px var(--mono);
    letter-spacing: 0.4em;
    text-align: center;
    height: 44px;
  }
</style>
