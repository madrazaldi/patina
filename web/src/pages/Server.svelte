<script lang="ts">
  import { ExternalLink, Search } from "@lucide/svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { ago, date } from "../lib/format";
  import { resource } from "../lib/resource.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { Pref } from "../lib/types";

  type Tab = "overview" | "libraries" | "maintenance" | "settings" | "users";
  const TABS: [Tab, string][] = [
    ["overview", "Overview"],
    ["libraries", "Libraries"],
    ["maintenance", "Maintenance"],
    ["settings", "Settings"],
    ["users", "Users & devices"],
  ];
  let tab = $state<Tab>("overview");

  const overview = resource(() => (tab === "overview" ? api.serverOverview() : Promise.resolve(null)));
  const butler = resource(() => (tab === "maintenance" ? api.butlerTasks() : Promise.resolve(null)));
  const prefs = resource(() => (tab === "settings" ? api.prefs() : Promise.resolve(null)));
  const people = resource(() => (tab === "users" ? api.usersDevices() : Promise.resolve(null)));

  let filter = $state("");
  let advanced = $state(false);
  /** Unsaved edits of text settings, by id. */
  let edits = $state<Record<string, string>>({});

  let shown = $derived.by(() => {
    const f = filter.toLowerCase();
    return (prefs.data ?? []).filter(
      (p) =>
        (advanced || !p.advanced) &&
        (!f || [p.label, p.id, p.summary ?? ""].some((s) => s.toLowerCase().includes(f))),
    );
  });

  async function save(p: Pref, value: string) {
    if (await act(api.setPref(p.id, value), "Setting saved")) {
      p.value = value;
      delete edits[p.id];
    }
  }

  async function checkUpdates() {
    if (await act(api.checkForUpdates(), "Checked for updates")) overview.reload();
  }

  const on = (v: string) => v === "true" || v === "1";
</script>

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
        <Status loading={!overview.error} error={overview.error} />
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
      {/if}
    {:else if tab === "libraries"}
      <p class="muted intro">Scan for new files, re-download metadata, analyze media or empty the trash.</p>
      <div class="rows">
        {#each app.sections as s (s.key)}
          {@const id = s.key ?? ""}
          <div class="setting">
            <div class="label">
              <b>{s.title}</b>
              <span class="selectable">{s.Location.map((l) => l.path).join(", ")}</span>
              {#if s.scannedAt}<span>Last scanned {ago(s.scannedAt)}</span>{/if}
            </div>
            <div class="control">
              <button class="btn sm" onclick={() => act(api.sectionAction(id, "scan"), "Scanning…")}>Scan</button>
              <button class="btn sm" title="Re-download all metadata" onclick={() => act(api.sectionAction(id, "refresh"), "Refreshing all metadata…")}>Refresh</button>
              <button class="btn sm" onclick={() => act(api.sectionAction(id, "analyze"), "Analyzing…")}>Analyze</button>
              <button class="btn sm" onclick={() => act(api.sectionAction(id, "emptyTrash"), "Trash emptied")}>Empty trash</button>
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
      <div class="section-head"><h2>Scheduled tasks</h2></div>
      <p class="muted intro">Plex's butler runs these during the maintenance window. You can start any of them now.</p>
      {#if !butler.data}
        <Status loading={!butler.error} error={butler.error} />
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
        <Status loading={!prefs.error} error={prefs.error} />
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
            <div class="pref">
              <div class="label">
                <b>{p.label}</b>
                <code>{p.id}</code>
                {#if p.summary}<span>{p.summary}</span>{/if}
              </div>
              <div class="control">
                {#if p.kind === "bool"}
                  <button class={["toggle", { on: on(p.value) }]} aria-label={p.label} onclick={() => save(p, on(p.value) ? "false" : "true")}></button>
                {:else if p.options.length}
                  <select class="select" value={p.value} onchange={(e) => save(p, e.currentTarget.value)}>
                    {#each p.options as [v, l] (v)}<option value={v}>{l}</option>{/each}
                  </select>
                {:else}
                  <input
                    class="input"
                    value={edits[p.id] ?? p.value}
                    oninput={(e) => (edits[p.id] = e.currentTarget.value)}
                    onkeydown={(e) => e.key === "Enter" && edits[p.id] !== undefined && save(p, edits[p.id])}
                  />
                  {#if edits[p.id] !== undefined && edits[p.id] !== p.value}
                    <button class="btn sm primary" onclick={() => save(p, edits[p.id])}>Save</button>
                  {/if}
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else}
      {@const d = people.data}
      {#if !d}
        <Status loading={!people.error} error={people.error} />
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
  .pref {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(200px, 320px);
    gap: 24px;
    align-items: center;
    padding: 12px 0;
  }
  .pref b {
    font-weight: 500;
  }
  .pref code {
    margin-left: 8px;
    font: 11px var(--mono);
    color: var(--text-3);
  }
  .pref span {
    display: block;
    margin-top: 3px;
    font-size: 12px;
    color: var(--text-3);
  }
  .pref .control {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .pref .input,
  .pref .select {
    width: 100%;
  }
  .devices {
    max-height: 480px;
    overflow: auto;
  }
</style>
