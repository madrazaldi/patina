<script lang="ts">
  // Request a movie, or seasons of a show, from Seerr. Users allowed advanced
  // requests also choose the Radarr/Sonarr server, quality profile and folder.
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Label } from "$lib/components/ui/label";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { bytes } from "../lib/format";
  import { openSeasons } from "../lib/seerr";
  import { app, captureScope, message } from "../lib/state.svelte";
  import type { SeerrCan, SeerrDetails, SeerrServer } from "../lib/types";

  let { details: d, is4k, can, onclose, ondone }: { details: SeerrDetails; is4k: boolean; can: SeerrCan; onclose: () => void; ondone: () => void } =
    $props();

  let seasons = $derived(d.mediaType === "tv" ? openSeasons(d, is4k) : []);
  let picked = $state<number[]>([]);
  let busy = $state(false);
  const guard = captureScope();

  // Advanced options: servers of the right kind and quality, with their defaults preselected.
  let servers = $state<SeerrServer[] | null>(null);
  let optionsError = $state<string>();
  let serverId = $state<number | null>(null);
  let profileId = $state<number | null>(null);
  let rootFolder = $state<string | null>(null);
  let server = $derived(servers?.find((s) => s.id === serverId) ?? null);

  $effect(() => {
    if (!can.advanced) return;
    api.seerrRequestOptions(d.mediaType, is4k).then(
      (list) => {
        servers = list;
        choose(list.find((s) => s.isDefault) ?? list[0] ?? null);
      },
      (e) => (optionsError = message(e)),
    );
  });

  function choose(s: SeerrServer | null) {
    serverId = s?.id ?? null;
    profileId = s?.activeProfileId ?? s?.profiles[0]?.id ?? null;
    rootFolder = s?.activeDirectory ?? s?.rootFolders[0]?.path ?? null;
  }

  const toggle = (n: number, on: boolean) => (picked = on ? [...picked, n].sort((a, b) => a - b) : picked.filter((x) => x !== n));
  let all = $derived(seasons.length > 0 && picked.length === seasons.length);

  const close = () => !busy && onclose();

  async function submit() {
    if (!guard.valid()) return onclose();
    busy = true;
    try {
      const state = await api.seerrRequest({
        mediaType: d.mediaType,
        tmdbId: d.tmdbId,
        tvdbId: d.tvdbId,
        seasons: d.mediaType === "tv" ? picked : undefined,
        is4k,
        serverId: server ? serverId : null,
        profileId: server ? profileId : null,
        rootFolder: server ? rootFolder : null,
      });
      app.toast(state === "approved" ? `${d.title} requested and approved` : `${d.title} requested`);
      busy = false;
      ondone();
    } catch (e) {
      busy = false;
      app.error(e);
    }
  }
</script>

<dialog class={["modal", d.mediaType === "tv" && "wide"]} use:modal={close} aria-labelledby="seerr-request-title">
  <h2 id="seerr-request-title">Request {d.title}{is4k ? " in 4K" : ""}</h2>
  <p class="desc">
    {#if d.mediaType === "tv"}
      Choose the seasons to add to the server.
    {:else}
      Seerr asks Radarr to find it and adds it to the server once downloaded.
    {/if}
  </p>

  {#if d.mediaType === "tv"}
    <div class="rows max-h-80 overflow-y-auto">
      <label class="list-row cursor-pointer grid-cols-[auto_1fr]">
        <Checkbox checked={all} indeterminate={picked.length > 0 && !all} onCheckedChange={(on) => (picked = on ? seasons.map((s) => s.number) : [])} disabled={busy} data-initial-focus />
        <span class="font-medium">All seasons</span>
      </label>
      {#each seasons as s (s.number)}
        <label class="list-row cursor-pointer grid-cols-[auto_1fr_auto]">
          <Checkbox checked={picked.includes(s.number)} onCheckedChange={(on) => toggle(s.number, on)} disabled={busy} aria-label={s.name} />
          <span class="truncate">{s.name}</span>
          <span class="text-muted-foreground text-xs tabular-nums">{s.episodes != null ? `${s.episodes} episodes` : ""}{s.airDate ? ` · ${s.airDate.slice(0, 4)}` : ""}</span>
        </label>
      {/each}
    </div>
  {/if}

  {#if can.advanced}
    {#if optionsError}
      <p class="text-destructive mt-4 text-xs">{optionsError}</p>
    {:else if servers && servers.length}
      <div class="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-3">
        <div class="flex flex-col gap-1.5">
          <Label class="text-muted-foreground text-xs" for="seerr-server">{d.mediaType === "tv" ? "Sonarr" : "Radarr"} server</Label>
          <NativeSelect.Root id="seerr-server" class="w-full" value={String(serverId)} disabled={busy} onchange={(e) => choose(servers?.find((s) => s.id === Number(e.currentTarget.value)) ?? null)}>
            {#each servers as s (s.id)}<NativeSelect.Option value={String(s.id)}>{s.name}{s.isDefault ? " (default)" : ""}</NativeSelect.Option>{/each}
          </NativeSelect.Root>
        </div>
        <div class="flex flex-col gap-1.5">
          <Label class="text-muted-foreground text-xs" for="seerr-profile">Quality profile</Label>
          <NativeSelect.Root id="seerr-profile" class="w-full" value={String(profileId)} disabled={busy} onchange={(e) => (profileId = Number(e.currentTarget.value))}>
            {#each server?.profiles ?? [] as p (p.id)}<NativeSelect.Option value={String(p.id)}>{p.name}</NativeSelect.Option>{/each}
          </NativeSelect.Root>
        </div>
        <div class="flex flex-col gap-1.5">
          <Label class="text-muted-foreground text-xs" for="seerr-folder">Folder</Label>
          <NativeSelect.Root id="seerr-folder" class="w-full" value={rootFolder ?? ""} disabled={busy} onchange={(e) => (rootFolder = e.currentTarget.value)}>
            {#each server?.rootFolders ?? [] as f (f.path)}<NativeSelect.Option value={f.path}>{f.path}{f.freeSpace != null ? ` (${bytes(f.freeSpace)} free)` : ""}</NativeSelect.Option>{/each}
          </NativeSelect.Root>
        </div>
      </div>
    {/if}
  {/if}

  <div class="foot">
    <Button variant="outline" disabled={busy} onclick={close}>Cancel</Button>
    <Button disabled={busy || (d.mediaType === "tv" && picked.length === 0)} onclick={submit} data-initial-focus={d.mediaType === "movie" ? true : undefined}>
      Request{d.mediaType === "tv" && picked.length ? ` ${picked.length} season${picked.length === 1 ? "" : "s"}` : ""}
    </Button>
  </div>
</dialog>
