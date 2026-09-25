<script lang="ts">
  import { Ellipsis, ExternalLink, Plus, RefreshCw, ScanSearch, Search, Trash, Trash2 } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import ServerLibraryDialog from "../components/ServerLibraryDialog.svelte";
  import ServerLibraryPrefs from "../components/ServerLibraryPrefs.svelte";
  import ServerPrefRow from "../components/ServerPrefRow.svelte";
  import StatsBandwidth from "../components/StatsBandwidth.svelte";
  import StatsRanking from "../components/StatsRanking.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { ago, bytes, count, date, initials } from "../lib/format";
  import { img } from "../lib/images";
  import { resource } from "../lib/resource.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { Directory, Pref } from "../lib/types";

  type Tab = "overview" | "libraries" | "maintenance" | "settings" | "users";
  const TABS: [Tab, string][] = [
    ["overview", "Overview"],
    ["libraries", "Libraries"],
    ["maintenance", "Maintenance"],
    ["settings", "Settings"],
    ["users", "Users & devices"],
  ];
  let tab = $state<Tab>("overview");

  type Period = "day" | "month";
  const PERIODS: [Period, string][] = [
    ["day", "Last 24 hours"],
    ["month", "Last 30 days"],
  ];
  let period = $state<Period>("day");

  const overview = resource(() => (tab === "overview" ? api.serverOverview() : Promise.resolve(null)));
  const stats = resource(() => {
    if (tab !== "overview") return Promise.resolve(null);
    const now = Math.floor(Date.now() / 1000);
    // Hourly buckets for a day, daily ones for a month.
    const [timespan, step] = period === "day" ? [6, 3600] : [4, 86400];
    const since = period === "day" ? now - 86400 : now - 30 * 86400;
    return api.serverStats(timespan, since).then((s) => ({ s, since, step }));
  });
  const butler = resource(() => (tab === "maintenance" ? api.butlerTasks() : Promise.resolve(null)));
  const prefs = resource(() => (tab === "settings" ? api.prefs() : Promise.resolve(null)));
  const people = resource(() => (tab === "users" ? api.usersDevices() : Promise.resolve(null)));

  let filter = $state("");
  let advanced = $state(false);

  let shown = $derived.by(() => {
    const f = filter.toLowerCase();
    return (prefs.data ?? []).filter(
      (p) =>
        (advanced || !p.advanced) &&
        (!f || [p.label, p.id, p.summary ?? ""].some((s) => s.toLowerCase().includes(f))),
    );
  });

  async function save(p: Pref, value: string) {
    const ok = await act(api.setPref(p.id, value), "Setting saved");
    if (ok) p.value = value;
    return ok;
  }

  async function checkUpdates() {
    if (await act(api.checkForUpdates(), "Checked for updates")) overview.reload();
  }

  /** "12.5" hours below ten, whole hours above. */
  const hours = (secs: number) => {
    const h = secs / 3600;
    return h < 10 ? h.toFixed(1) : count(Math.round(h));
  };

  // Libraries: the add/edit dialog, per-library settings and the "…" menu.
  let libraryDialog = $state<{ section?: Directory } | null>(null);
  let prefsFor = $state<Directory | null>(null);
  let menu = $state<{ section: Directory; x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent, section: Directory) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { section, x: Math.max(8, r.right - 220), y: Math.min(r.bottom + 4, window.innerHeight - 150) };
  }

  function run(action: "refresh" | "analyze" | "emptyTrash", ok: string) {
    const id = menu?.section.key ?? "";
    menu = null;
    act(api.sectionAction(id, action), ok);
  }

  async function remove(s: Directory) {
    menu = null;
    const ok = await app.confirm(`Delete “${s.title}”?`, {
      text: "The library and its metadata are removed from the server. Media files on disk are not touched.",
      action: "Delete library",
      danger: true,
    });
    if (ok) act(api.deleteLibrary(s.key ?? ""), `Deleted “${s.title}”`);
  }

  // Diagnostics can take a while on big servers.
  let saving = $state<"logs" | "database" | null>(null);
  async function diagnostics(what: "logs" | "database") {
    saving = what;
    try {
      app.toast(`Saved to ${await api.saveDiagnostics(what)}`);
    } catch (e) {
      app.error(e);
    } finally {
      saving = null;
    }
  }
</script>
{#if people.error && people.data}<Status error={people.error} retry={() => people.reload()} stale />{/if}
{#if prefs.error && prefs.data}<Status error={prefs.error} retry={() => prefs.reload()} stale />{/if}
{#if butler.error && butler.data}<Status error={butler.error} retry={() => butler.reload()} stale />{/if}
{#if stats.error && stats.data}<Status error={stats.error} retry={() => stats.reload()} stale />{/if}
{#if overview.error && overview.data}<Status error={overview.error} retry={() => overview.reload()} stale />{/if}

<svelte:window onkeydown={(e) => e.key === "Escape" && (menu = null)} />

{#if !app.owned}
  <Status title="Server tools need owner access" text="This server is shared with you." fill />
{:else}
  <div class="page">
    <div class="page-head">
      <div>
        <h1>{app.session?.serverName}</h1>
        <div class="sub">Manage your Plex Media Server.</div>
      </div>
      <div class="tabs">
        {#each TABS as [t, label] (t)}
          <button class={["tab", { active: tab === t }]} onclick={() => (tab = t)}>{label}</button>
        {/each}
      </div>
    </div>

    {#if tab === "overview"}
      {@const o = overview.data}
      {#if !o}
        <Status loading={!overview.error} error={overview.error} retry={() => overview.reload()} />
      {:else}
        <div class="stats num">
          <div><b>{o.hostCpu.toFixed(0)}%</b><span>Host CPU</span></div>
          <div><b>{o.plexCpu.toFixed(0)}%</b><span>Plex CPU</span></div>
          <div><b>{o.hostMemory.toFixed(0)}%</b><span>Host memory</span></div>
          <div><b>{o.plexMemory.toFixed(0)}%</b><span>Plex memory</span></div>
        </div>
        <div class="columns">
          <section>
            <div class="section-head"><h2>Server</h2></div>
            <dl class="kv">
              <dt>Version</dt><dd>{o.version ?? "—"}</dd>
              <dt>Platform</dt><dd>{o.platform ?? "—"}</dd>
              <dt>Owner</dt><dd>{o.owner ?? "—"}</dd>
              <dt>Plex Pass</dt><dd>{o.plexPass ? "Active" : "No"}</dd>
              <dt>Connected via</dt><dd class="selectable">{o.connectedVia}</dd>
              <dt>Machine ID</dt><dd class="selectable">{o.machineIdentifier ?? "—"}</dd>
              <dt>Transcoder</dt><dd>{o.transcoder ? "Available" : "—"}</dd>
            </dl>
          </section>
          <section>
            <div class="section-head"><h2>Updates</h2></div>
            {#if o.release}
              {@const release = o.release}
              <p>Version {release.version} is available.</p>
              {#if release.added}<p class="notes faint">{release.added}</p>{/if}
              <div class="row-flex buttons">
                {#if o.canInstall}
                  <button class="btn sm primary" onclick={() => act(api.applyUpdate(false), "Installing update…")}>Install now</button>
                  <button class="btn sm" onclick={() => act(api.applyUpdate(true), "Update scheduled for tonight")}>Tonight</button>
                {:else if release.downloadUrl}
                  <button class="btn sm" onclick={() => release.downloadUrl && api.openUrl(release.downloadUrl)}>
                    <ExternalLink size={14} />Download
                  </button>
                {/if}
              </div>
            {:else}
              <p class="muted">The server is up to date.</p>
            {/if}
            <button class="btn sm quiet check" onclick={checkUpdates}>Check now</button>
          </section>
        </div>

        <div class="section-head streaming-head">
          <h2>Streaming</h2>
          <div class="row-flex">
            {#each PERIODS as [p, label] (p)}
              <button class={["chip", { active: period === p }]} onclick={() => (period = p)}>{label}</button>
            {/each}
          </div>
        </div>
        {#if !stats.data}
          <Status loading={!stats.error} error={stats.error} retry={() => stats.reload()} />
        {:else}
          {@const { s, since, step } = stats.data}
          <div class={["streaming", { stale: stats.loading }]}>
            {#if s.bandwidth.length === 0 && s.plays === 0}
              <p class="faint">Nothing was streamed from this server {period === "day" ? "in the last 24 hours" : "in the last 30 days"}.</p>
            {:else}
              <div class="stats">
                <div><b>{bytes(s.bandwidth.reduce((n, b) => n + b.lan + b.wan, 0))}</b><span>Data sent</span></div>
                <div><b>{count(s.plays)}</b><span>Plays</span></div>
                <div><b>{hours(s.playSeconds)}</b><span>Hours played</span></div>
              </div>
              {#if s.bandwidth.length}
                <div class="chart"><StatsBandwidth buckets={s.bandwidth} {since} {step} /></div>
              {/if}
              <div class="columns">
                <section>
                  <div class="section-head"><h2>Top users</h2></div>
                  <StatsRanking rows={s.accounts} empty="No data sent to users." />
                </section>
                <section>
                  <div class="section-head"><h2>Top devices</h2></div>
                  <StatsRanking rows={s.devices} empty="No data sent to devices." />
                </section>
              </div>
            {/if}
          </div>
        {/if}
      {/if}
    {:else if tab === "libraries"}
      <div class="lead">
        <p class="muted">Scan for new files, change folders and settings, or add a library.</p>
        <button class="btn sm" onclick={() => (libraryDialog = {})}><Plus size={14} />Add library</button>
      </div>
      {#if app.sections.length === 0}<p class="faint">This server has no libraries yet.</p>{/if}
      <div class="rows">
        {#each app.sections as s (s.key)}
          {@const id = s.key ?? ""}
          {@const task = app.activities.find((a) => a.sectionId === id)}
          <div class="setting">
            <div class="label grow">
              <b>{s.title}</b>
              <span class="selectable">{s.Location.map((l) => l.path).join(", ")}</span>
              {#if task}
                <span class="num">{task.subtitle || task.title} · {task.progress}%</span>
                <div class="bar scan"><i style:width="{task.progress}%"></i></div>
              {:else if s.refreshing}
                <span>Scanning…</span>
              {:else if s.scannedAt}
                <span>Last scanned {ago(s.scannedAt)}</span>
              {/if}
            </div>
            <div class="control">
              {#if task || s.refreshing}
                <button class="btn sm" onclick={() => act(api.cancelScan(id), "Scan cancelled")}>Cancel</button>
              {:else}
                <button class="btn sm" onclick={() => act(api.sectionAction(id, "scan"), "Scanning…")}>Scan</button>
              {/if}
              <button class="btn sm" onclick={() => (libraryDialog = { section: s })}>Edit</button>
              <button class="btn sm" onclick={() => (prefsFor = s)}>Settings</button>
              <button class="icon-btn" title="More" aria-label="More actions for {s.title}" onclick={(e) => openMenu(e, s)}>
                <Ellipsis size={16} />
              </button>
            </div>
          </div>
        {/each}
      </div>
      <div class="row-flex buttons">
        <button class="btn sm primary" onclick={() => act(api.serverAction("scanAll"), "Scanning all libraries…")}>Scan all</button>
        <button class="btn sm quiet" onclick={() => act(api.reloadSections())}>Reload list</button>
      </div>
    {:else if tab === "maintenance"}
      <div class="rows">
        <div class="setting">
          <div class="label"><b>Clean bundles</b><span>Delete metadata bundles for media that no longer exists.</span></div>
          <div class="control"><button class="btn sm" onclick={() => act(api.serverAction("cleanBundles"), "Cleaning bundles…")}>Run</button></div>
        </div>
        <div class="setting">
          <div class="label"><b>Optimize database</b><span>Compact and re-index the library database.</span></div>
          <div class="control">
            <button class="btn sm" onclick={() => act(api.serverAction("optimizeDatabase"), "Optimizing database…")}>Run</button>
          </div>
        </div>
      </div>

      <div class="section-head"><h2>Diagnostics</h2></div>
      <div class="rows">
        {#each [
          ["logs", "Download logs", "Server logs as a zip, for troubleshooting or a support request."],
          ["database", "Download database backup", "A copy of the library database. Large libraries take a while."],
        ] as const as [what, label, desc] (what)}
          <div class="setting">
            <div class="label"><b>{label}</b><span>{desc}</span></div>
            <div class="control">
              {#if saving === what}<div class="spinner sm"></div>{/if}
              <button class="btn sm" disabled={saving !== null} onclick={() => diagnostics(what)}>
                {saving === what ? "Saving…" : "Download"}
              </button>
            </div>
          </div>
        {/each}
      </div>

      <div class="section-head">
        <h2>Scheduled tasks</h2>
        <div class="row-flex">
          <button class="btn sm quiet" onclick={() => act(api.butlerAll(true), "Started all tasks")}>Start all</button>
          <button class="btn sm quiet" onclick={() => act(api.butlerAll(false), "Stopped all tasks")}>Stop all</button>
        </div>
      </div>
      <p class="muted intro">Plex's butler runs these during the maintenance window. You can start any of them now.</p>
      {#if !butler.data}
        <Status loading={!butler.error} error={butler.error} retry={() => butler.reload()} />
      {:else}
        <div class="rows">
          {#each butler.data as t, i (t.name ?? i)}
            {@const name = t.name ?? ""}
            <div class="setting">
              <div class="label"><b>{t.title ?? name}</b><span>{t.description ?? ""}</span></div>
              <div class="control">
                {#if !t.enabled}<span class="tag">Disabled</span>{/if}
                <button class="btn sm" onclick={() => act(api.butlerTask(name, true), "Task started")}>Start</button>
                <button class="btn sm quiet" onclick={() => act(api.butlerTask(name, false), "Task stopped")}>Stop</button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else if tab === "settings"}
      {#if !prefs.data}
        <Status loading={!prefs.error} error={prefs.error} retry={() => prefs.reload()} />
      {:else}
        <div class="toolbar">
          <label class="filter">
            <Search size={14} />
            <input placeholder="Filter settings" bind:value={filter} />
          </label>
          <button class={["chip", { active: advanced }]} onclick={() => (advanced = !advanced)}>Show advanced</button>
          <span class="faint num">{shown.length} settings</span>
        </div>
        <div class="rows">
          {#each shown as p (p.id)}
            <ServerPrefRow pref={p} onsave={(v) => save(p, v)} />
          {/each}
        </div>
      {/if}
    {:else}
      {@const d = people.data}
      {#if !d}
        <Status loading={!people.error} error={people.error} retry={() => people.reload()} />
      {:else}
        <div class="columns">
          <section>
            <div class="section-head"><h2>Accounts</h2></div>
            <div class="rows">
              {#each d.accounts.filter((a) => (a.id ?? 0) > 0) as a (a.id)}
                <div class="setting">
                  <div class="label">
                    <b>{a.name}</b>
                    <span>Audio: {a.defaultAudioLanguage ?? "auto"} · Subtitles: {a.defaultSubtitleLanguage ?? "auto"}</span>
                  </div>
                </div>
              {/each}
            </div>
          </section>
          <section>
            <div class="section-head"><h2>Players on the network</h2></div>
            {#if d.clients.length === 0}<p class="faint">No players are advertising on the local network right now.</p>{/if}
            <div class="rows">
              {#each d.clients as c, i (c.machineIdentifier ?? i)}
                <div class="setting">
                  <div class="label"><b>{c.name}</b><span>{c.product ?? ""} · {c.address ?? ""}</span></div>
                </div>
              {/each}
            </div>
          </section>
        </div>
        <div class="section-head"><h2>Friends & shared users</h2></div>
        {#if d.friends.length === 0}<p class="faint">This server isn't shared with anyone.</p>{/if}
        <div class="rows">
          {#each d.friends as f, i (f.id ?? i)}
            {@const name = f.title || f.username || f.email || "User"}
            {@const detail = [f.username !== name ? f.username : undefined, f.email].filter(Boolean).join(" · ")}
            <div class="setting">
              <div class="label row-flex">
                <div class="avatar">{#if f.thumb}<Img src={img(f.thumb, 56, 56)} />{:else}{initials(name)}{/if}</div>
                <div>
                  <b>{name}</b>
                  {#if detail}<span class="selectable">{detail}</span>{/if}
                </div>
              </div>
              <div class="control">
                {#if f.home}<span class="tag">Home</span>{/if}
                {#if f.restricted}<span class="tag">Restricted</span>{/if}
                {#if f.status && f.status !== "accepted"}<span class="tag status">{f.status}</span>{/if}
              </div>
            </div>
          {/each}
        </div>
        <div class="section-head"><h2>Devices that have connected</h2></div>
        <div class="rows devices">
          {#each [...d.devices].reverse() as dev, i (dev.id ?? i)}
            <div class="setting">
              <div class="label"><b>{dev.name}</b><span>{dev.platform ?? ""}</span></div>
              <div class="control faint">{dev.createdAt ? `First seen ${date(dev.createdAt)}` : ""}</div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  {#if menu}
    {@const s = menu.section}
    <div class="scrim" role="presentation" onclick={() => (menu = null)}></div>
    <div class="menu" style:left="{menu.x}px" style:top="{menu.y}px">
      <button onclick={() => run("refresh", "Refreshing all metadata…")}><RefreshCw size={14} />Refresh all metadata</button>
      <button onclick={() => run("analyze", "Analyzing…")}><ScanSearch size={14} />Analyze media</button>
      <button onclick={() => run("emptyTrash", "Trash emptied")}><Trash2 size={14} />Empty trash</button>
      <hr />
      <button class="danger" onclick={() => remove(s)}><Trash size={14} />Delete library…</button>
    </div>
  {/if}

  {#if libraryDialog}
    <ServerLibraryDialog section={libraryDialog.section} onclose={() => (libraryDialog = null)} />
  {/if}
  {#if prefsFor}
    <ServerLibraryPrefs section={prefsFor} onclose={() => (prefsFor = null)} />
  {/if}
{/if}

<style>
  .columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 48px;
  }
  .intro {
    margin-bottom: 14px;
  }
  .lead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 14px;
  }
  .notes {
    margin-top: 10px;
    max-height: 160px;
    overflow: auto;
    white-space: pre-line;
    font-size: 12px;
  }
  .buttons {
    margin-top: 16px;
  }
  .check {
    margin-top: 12px;
    margin-left: -10px;
  }
  .streaming-head {
    margin-top: 48px;
    align-items: center;
  }
  .streaming {
    transition: opacity 0.15s var(--ease);
  }
  .streaming.stale {
    opacity: 0.5;
  }
  .chart {
    margin-top: 24px;
  }
  .bar.scan {
    max-width: 360px;
    margin-top: 8px;
  }
  .spinner.sm {
    width: 12px;
    height: 12px;
  }
  .status {
    text-transform: capitalize;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 320px;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--bg-2);
    color: var(--text-3);
  }
  .filter input {
    flex: 1;
    border: 0;
    outline: 0;
    background: none;
  }
  .devices {
    max-height: 480px;
    overflow: auto;
  }
</style>
