<script lang="ts">
  import { Ellipsis, ExternalLink, FolderOpen, Plus, RefreshCw, ScanSearch, Search, SearchX, Trash, Trash2, Users } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import * as Empty from "$lib/components/ui/empty";
  import * as InputGroup from "$lib/components/ui/input-group";
  import { Label } from "$lib/components/ui/label";
  import { Progress } from "$lib/components/ui/progress";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Switch } from "$lib/components/ui/switch";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
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
  import type { Directory, Pref, ScheduledTask } from "../lib/types";

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
  const maintenance = resource(() => (tab === "maintenance" ? api.maintenance() : Promise.resolve(null)));
  const prefs = resource(() => (tab === "settings" ? api.prefs() : Promise.resolve(null)));
  const people = resource(() => (tab === "users" ? api.usersDevices() : Promise.resolve(null)));

  // Settings: one group at a time, or every match while filtering.
  let filter = $state("");
  let advanced = $state(false);
  let group = $state<string>();
  let searching = $derived(filter.trim() !== "");

  /** Groups with the settings that pass the advanced switch and the filter. */
  let groups = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    return (prefs.data ?? [])
      .map((g) => ({
        ...g,
        prefs: g.prefs.filter(
          (p) =>
            (advanced || !p.advanced) &&
            (!f || [p.label, p.id, p.summary ?? ""].some((s) => s.toLowerCase().includes(f))),
        ),
      }))
      .filter((g) => g.prefs.length);
  });
  let current = $derived(groups.find((g) => g.id === group) ?? groups[0]);
  let shownGroups = $derived(searching ? groups : current ? [current] : []);

  async function save(p: Pref, value: string) {
    const ok = await act(api.setPref(p.id, value), "Setting saved");
    if (ok) p.value = value;
    return ok;
  }

  const isOn = (v: string) => v === "true" || v === "1";

  /** Switch a task's scheduled runs on or off. */
  async function toggleTask(t: ScheduledTask) {
    if (!t.pref) return;
    const next = isOn(t.pref.value) ? "false" : "true";
    if (await save(t.pref, next)) t.enabled = next === "true";
  }

  const every = (days?: number) => (!days ? "" : days === 1 ? "Daily" : `Every ${days} days`);

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
{#if maintenance.error && maintenance.data}<Status error={maintenance.error} retry={() => maintenance.reload()} stale />{/if}
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
        <div class="flex flex-col gap-6">
          <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
            {#each [["Host CPU", o.hostCpu], ["Plex CPU", o.plexCpu], ["Host memory", o.hostMemory], ["Plex memory", o.plexMemory]] as const as [label, value] (label)}
              <Card.Root size="sm">
                <Card.Header>
                  <Card.Description>{label}</Card.Description>
                  <Card.Title class="text-2xl font-semibold tracking-tight tabular-nums group-data-[size=sm]/card:text-2xl">{value.toFixed(0)}%</Card.Title>
                </Card.Header>
                <Card.Content>
                  <Progress {value} class="[&_[data-slot=progress-indicator]]:bg-tint" />
                </Card.Content>
              </Card.Root>
            {/each}
          </div>

          <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <Card.Root>
              <Card.Header>
                <Card.Title><h2>Server</h2></Card.Title>
              </Card.Header>
              <Card.Content>
                <dl class="kv">
                  <dt>Version</dt><dd>{o.version ?? "—"}</dd>
                  <dt>Platform</dt><dd>{o.platform ?? "—"}</dd>
                  <dt>Owner</dt><dd>{o.owner ?? "—"}</dd>
                  <dt>Plex Pass</dt><dd>{o.plexPass ? "Active" : "No"}</dd>
                  <dt>Connected via</dt><dd class="selectable">{o.connectedVia}</dd>
                  <dt>Machine ID</dt><dd class="selectable">{o.machineIdentifier ?? "—"}</dd>
                  <dt>Transcoder</dt><dd>{o.transcoder ? "Available" : "—"}</dd>
                </dl>
              </Card.Content>
            </Card.Root>
            <Card.Root>
              <Card.Header>
                <Card.Title><h2>Updates</h2></Card.Title>
              </Card.Header>
              <Card.Content class="flex-1">
                {#if o.release}
                  {@const release = o.release}
                  <p>Version {release.version} is available.</p>
                  {#if release.added}<p class="text-muted-foreground mt-2.5 max-h-40 overflow-auto text-xs whitespace-pre-line">{release.added}</p>{/if}
                  <div class="mt-4 flex items-center gap-2">
                    {#if o.canInstall}
                      <Button size="sm" onclick={() => act(api.applyUpdate(false), "Installing update…")}>Install now</Button>
                      <Button variant="outline" size="sm" onclick={() => act(api.applyUpdate(true), "Update scheduled for tonight")}>Tonight</Button>
                    {:else if release.downloadUrl}
                      <Button variant="outline" size="sm" onclick={() => release.downloadUrl && api.openUrl(release.downloadUrl)}>
                        <ExternalLink />Download
                      </Button>
                    {/if}
                  </div>
                {:else}
                  <p class="text-muted-foreground">The server is up to date.</p>
                {/if}
              </Card.Content>
              <Card.Footer class="border-t">
                <Button variant="outline" size="sm" onclick={checkUpdates}>Check now</Button>
              </Card.Footer>
            </Card.Root>
          </div>

          <Card.Root>
            <Card.Header>
              <Card.Title><h2>Streaming</h2></Card.Title>
              <Card.Action>
                <ToggleGroup.Root type="single" variant="outline" size="sm" bind:value={() => period, (v) => v && (period = v as Period)}>
                  {#each PERIODS as [p, label] (p)}
                    <ToggleGroup.Item value={p}>{label}</ToggleGroup.Item>
                  {/each}
                </ToggleGroup.Root>
              </Card.Action>
            </Card.Header>
            <Card.Content>
              {#if !stats.data}
                <Status loading={!stats.error} error={stats.error} retry={() => stats.reload()} />
              {:else}
                {@const { s, since, step } = stats.data}
                <div class={["flex flex-col gap-6 transition-opacity", { "opacity-50": stats.loading }]}>
                  {#if s.bandwidth.length === 0 && s.plays === 0}
                    <Empty.Root class="border">
                      <Empty.Header>
                        <Empty.Media variant="icon"><SearchX /></Empty.Media>
                        <Empty.Title>No streaming</Empty.Title>
                        <Empty.Description>Nothing was streamed from this server {period === "day" ? "in the last 24 hours" : "in the last 30 days"}.</Empty.Description>
                      </Empty.Header>
                    </Empty.Root>
                  {:else}
                    <div class="grid grid-cols-3 gap-4">
                      {#each [[bytes(s.bandwidth.reduce((n, b) => n + b.lan + b.wan, 0)), "Data sent"], [count(s.plays), "Plays"], [hours(s.playSeconds), "Hours played"]] as [value, label] (label)}
                        <div class="rounded-lg border px-4 py-3">
                          <div class="text-muted-foreground text-xs font-medium">{label}</div>
                          <div class="mt-1 text-2xl leading-tight font-semibold tracking-tight tabular-nums">{value}</div>
                        </div>
                      {/each}
                    </div>
                    {#if s.bandwidth.length}
                      <StatsBandwidth buckets={s.bandwidth} {since} {step} />
                    {/if}
                  {/if}
                </div>
              {/if}
            </Card.Content>
          </Card.Root>

          {#if stats.data && (stats.data.s.bandwidth.length > 0 || stats.data.s.plays > 0)}
            {@const s = stats.data.s}
            <div class={["grid grid-cols-1 gap-6 transition-opacity lg:grid-cols-2", { "opacity-50": stats.loading }]}>
              <Card.Root>
                <Card.Header><Card.Title><h2>Top users</h2></Card.Title></Card.Header>
                <Card.Content><StatsRanking rows={s.accounts} empty="No data sent to users." /></Card.Content>
              </Card.Root>
              <Card.Root>
                <Card.Header><Card.Title><h2>Top devices</h2></Card.Title></Card.Header>
                <Card.Content><StatsRanking rows={s.devices} empty="No data sent to devices." /></Card.Content>
              </Card.Root>
            </div>
          {/if}
        </div>
      {/if}
    {:else if tab === "libraries"}
      <div class="mb-4 flex items-center justify-between gap-4">
        <p class="text-muted-foreground">Scan for new files, change folders and settings, or add a library.</p>
        <Button variant="outline" size="sm" onclick={() => (libraryDialog = {})}><Plus />Add library</Button>
      </div>
      {#if app.sections.length === 0}
        <Empty.Root class="border">
          <Empty.Header>
            <Empty.Media variant="icon"><FolderOpen /></Empty.Media>
            <Empty.Title>No libraries</Empty.Title>
            <Empty.Description>This server has no libraries yet.</Empty.Description>
          </Empty.Header>
        </Empty.Root>
      {:else}
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
                  <Progress value={task.progress} class="mt-2 max-w-90 [&_[data-slot=progress-indicator]]:bg-tint" />
                {:else if s.refreshing}
                  <span>Scanning…</span>
                {:else if s.scannedAt}
                  <span>Last scanned {ago(s.scannedAt)}</span>
                {/if}
              </div>
              <div class="control">
                {#if task || s.refreshing}
                  <Button variant="outline" size="sm" onclick={() => act(api.cancelScan(id), "Scan cancelled")}>Cancel</Button>
                {:else}
                  <Button variant="outline" size="sm" onclick={() => act(api.sectionAction(id, "scan"), "Scanning…")}>Scan</Button>
                {/if}
                <Button variant="outline" size="sm" onclick={() => (libraryDialog = { section: s })}>Edit</Button>
                <Button variant="outline" size="sm" onclick={() => (prefsFor = s)}>Settings</Button>
                <Button variant="ghost" size="icon-sm" title="More" aria-label="More actions for {s.title}" onclick={(e) => openMenu(e, s)}>
                  <Ellipsis />
                </Button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
      <div class="mt-4 flex items-center gap-2">
        <Button size="sm" onclick={() => act(api.serverAction("scanAll"), "Scanning all libraries…")}>Scan all</Button>
        <Button variant="ghost" size="sm" onclick={() => act(api.reloadSections())}>Reload list</Button>
      </div>
    {:else if tab === "maintenance"}
      <div class="flex flex-col gap-6">
        <Card.Root class="gap-0 py-0">
          <Card.Header class="border-b py-4">
            <Card.Title><h2>Database</h2></Card.Title>
          </Card.Header>
          <div class="divide-y">
            <div class="setting px-4">
              <div class="label"><b>Clean bundles</b><span>Delete metadata bundles for media that no longer exists.</span></div>
              <div class="control"><Button variant="outline" size="sm" onclick={() => act(api.serverAction("cleanBundles"), "Cleaning bundles…")}>Run</Button></div>
            </div>
            <div class="setting px-4">
              <div class="label"><b>Optimize database</b><span>Compact and re-index the library database.</span></div>
              <div class="control">
                <Button variant="outline" size="sm" onclick={() => act(api.serverAction("optimizeDatabase"), "Optimizing database…")}>Run</Button>
              </div>
            </div>
          </div>
        </Card.Root>

        <Card.Root class="gap-0 py-0">
          <Card.Header class="border-b py-4">
            <Card.Title><h2>Scheduled tasks</h2></Card.Title>
            <Card.Description>Plex's butler runs these during the maintenance window. Switch off any you don't want, or run one now.</Card.Description>
            <Card.Action class="flex items-center gap-1">
              <Button variant="ghost" size="sm" onclick={() => act(api.butlerAll(true), "Started all tasks")}>Start all</Button>
              <Button variant="ghost" size="sm" onclick={() => act(api.butlerAll(false), "Stopped all tasks")}>Stop all</Button>
            </Card.Action>
          </Card.Header>
          {#if !maintenance.data}
            <Status loading={!maintenance.error} error={maintenance.error} retry={() => maintenance.reload()} />
          {:else}
            {@const m = maintenance.data}
            <div class="divide-y">
              {#each m.schedule as p (p.id)}
                <ServerPrefRow pref={p} showId={false} onsave={(v) => save(p, v)} />
              {/each}
              {#each m.tasks as t, i (t.name ?? i)}
                {@const name = t.name ?? ""}
                {@const title = t.title ?? name}
                <div class="setting px-4">
                  <div class="label">
                    <b>{title}</b>
                    {#if t.description}<span>{t.description}</span>{/if}
                  </div>
                  <div class="control">
                    {#if t.interval}<span class="text-muted-foreground mr-2 text-xs">{every(t.interval)}</span>{/if}
                    <Button variant="outline" size="sm" onclick={() => act(api.butlerTask(name, true), `Started ${title}`)}>Run now</Button>
                    {#if t.pref}
                      <button
                        class={["toggle", { on: isOn(t.pref.value) }]}
                        title={isOn(t.pref.value) ? "Runs on schedule" : "Off"}
                        aria-label="Run {title} on schedule"
                        onclick={() => toggleTask(t)}
                      ></button>
                    {:else if !t.enabled}
                      <Badge variant="outline">Off</Badge>
                    {:else}
                      <span class="w-8"></span>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </Card.Root>

        <Card.Root class="gap-0 py-0">
          <Card.Header class="border-b py-4">
            <Card.Title><h2>Diagnostics</h2></Card.Title>
          </Card.Header>
          <div class="divide-y">
            {#each [
              ["logs", "Download logs", "Server logs as a zip, for troubleshooting or a support request."],
              ["database", "Download database backup", "A copy of the library database. Large libraries take a while."],
            ] as const as [what, label, desc] (what)}
              <div class="setting px-4">
                <div class="label"><b>{label}</b><span>{desc}</span></div>
                <div class="control">
                  {#if saving === what}<Spinner class="text-muted-foreground" />{/if}
                  <Button variant="outline" size="sm" disabled={saving !== null} onclick={() => diagnostics(what)}>
                    {saving === what ? "Saving…" : "Download"}
                  </Button>
                </div>
              </div>
            {/each}
          </div>
        </Card.Root>
      </div>
    {:else if tab === "settings"}
      {#if !prefs.data}
        <Status loading={!prefs.error} error={prefs.error} retry={() => prefs.reload()} />
      {:else}
        <div class="mb-6 flex items-center gap-4">
          <InputGroup.Root class="w-80">
            <InputGroup.Addon><Search /></InputGroup.Addon>
            <InputGroup.Input placeholder="Search all settings" bind:value={filter} />
          </InputGroup.Root>
          <div class="flex items-center gap-2">
            <Switch id="server-advanced" size="sm" bind:checked={advanced} />
            <Label for="server-advanced" class="text-muted-foreground font-normal">Show advanced</Label>
          </div>
        </div>
        {#if groups.length === 0}
          <Empty.Root class="border">
            <Empty.Header>
              <Empty.Media variant="icon"><SearchX /></Empty.Media>
              <Empty.Title>No results</Empty.Title>
              <Empty.Description>No settings match “{filter.trim()}”.</Empty.Description>
            </Empty.Header>
          </Empty.Root>
        {:else}
          <div class="grid grid-cols-[200px_minmax(0,1fr)] items-start gap-8">
            <nav class="groups sticky top-0 flex flex-col gap-1">
              {#each groups as g (g.id)}
                {@const active = !searching && g.id === current?.id}
                <Button
                  variant={active ? "secondary" : "ghost"}
                  class={["justify-between", { active }]}
                  onclick={() => ((group = g.id), (filter = ""))}
                >
                  {g.label}<span class="text-muted-foreground text-[11px] font-normal tabular-nums">{g.prefs.length}</span>
                </Button>
              {/each}
            </nav>
            <div class="flex min-w-0 flex-col gap-6">
              {#each shownGroups as g (g.id)}
                <Card.Root class="gap-0 py-0">
                  <Card.Header class="border-b py-4">
                    <Card.Title><h2>{g.label}</h2></Card.Title>
                  </Card.Header>
                  <div class="divide-y">
                    {#each g.prefs as p (p.id)}
                      <ServerPrefRow pref={p} showId={advanced} onsave={(v) => save(p, v)} />
                    {/each}
                  </div>
                </Card.Root>
              {/each}
            </div>
          </div>
        {/if}
      {/if}
    {:else}
      {@const d = people.data}
      {#if !d}
        <Status loading={!people.error} error={people.error} retry={() => people.reload()} />
      {:else}
        <div class="flex flex-col gap-6">
          <div class="grid grid-cols-1 items-start gap-6 lg:grid-cols-2">
            <Card.Root class="gap-0 py-0">
              <Card.Header class="border-b py-4"><Card.Title><h2>Accounts</h2></Card.Title></Card.Header>
              <div class="divide-y">
                {#each d.accounts.filter((a) => (a.id ?? 0) > 0) as a (a.id)}
                  <div class="setting px-4">
                    <div class="label">
                      <b>{a.name}</b>
                      <span>Audio: {a.defaultAudioLanguage ?? "auto"} · Subtitles: {a.defaultSubtitleLanguage ?? "auto"}</span>
                    </div>
                  </div>
                {/each}
              </div>
            </Card.Root>
            <Card.Root class="gap-0 py-0">
              <Card.Header class="border-b py-4"><Card.Title><h2>Players on the network</h2></Card.Title></Card.Header>
              {#if d.clients.length === 0}
                <Empty.Root class="border-0">
                  <Empty.Header>
                    <Empty.Media variant="icon"><Users /></Empty.Media>
                    <Empty.Description>No players are advertising on the local network right now.</Empty.Description>
                  </Empty.Header>
                </Empty.Root>
              {/if}
              <div class="divide-y">
                {#each d.clients as c, i (c.machineIdentifier ?? i)}
                  <div class="setting px-4">
                    <div class="label"><b>{c.name}</b><span>{c.product ?? ""} · {c.address ?? ""}</span></div>
                  </div>
                {/each}
              </div>
            </Card.Root>
          </div>

          <Card.Root class="gap-0 py-0">
            <Card.Header class="border-b py-4"><Card.Title><h2>Friends & shared users</h2></Card.Title></Card.Header>
            {#if d.friends.length === 0}
              <Empty.Root class="border-0">
                <Empty.Header>
                  <Empty.Media variant="icon"><Users /></Empty.Media>
                  <Empty.Description>This server isn't shared with anyone.</Empty.Description>
                </Empty.Header>
              </Empty.Root>
            {/if}
            <div class="divide-y">
              {#each d.friends as f, i (f.id ?? i)}
                {@const name = f.title || f.username || f.email || "User"}
                {@const detail = [f.username !== name ? f.username : undefined, f.email].filter(Boolean).join(" · ")}
                <div class="setting px-4">
                  <div class="label row-flex">
                    <div class="avatar">{#if f.thumb}<Img src={img(f.thumb, 56, 56)} />{:else}{initials(name)}{/if}</div>
                    <div>
                      <b>{name}</b>
                      {#if detail}<span class="selectable">{detail}</span>{/if}
                    </div>
                  </div>
                  <div class="control">
                    {#if f.home}<Badge variant="outline">Home</Badge>{/if}
                    {#if f.restricted}<Badge variant="outline">Restricted</Badge>{/if}
                    {#if f.status && f.status !== "accepted"}<Badge variant="secondary" class="capitalize">{f.status}</Badge>{/if}
                  </div>
                </div>
              {/each}
            </div>
          </Card.Root>

          <Card.Root class="gap-0 py-0">
            <Card.Header class="border-b py-4"><Card.Title><h2>Devices that have connected</h2></Card.Title></Card.Header>
            <div class="max-h-120 divide-y overflow-auto">
              {#each [...d.devices].reverse() as dev, i (dev.id ?? i)}
                <div class="setting px-4">
                  <div class="label"><b>{dev.name}</b><span>{dev.platform ?? ""}</span></div>
                  <div class="control text-muted-foreground text-sm">{dev.createdAt ? `First seen ${date(dev.createdAt)}` : ""}</div>
                </div>
              {/each}
            </div>
          </Card.Root>
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

