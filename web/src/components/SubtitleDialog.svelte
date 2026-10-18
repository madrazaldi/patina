<script lang="ts">
  // Online subtitle search for a movie or episode, shown while `app.subtitleTarget` is set.
  import { untrack } from "svelte";
  import { Download } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { Spinner } from "$lib/components/ui/spinner";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { act, app, captureScope, message } from "../lib/state.svelte";
  import type { Metadata, Stream } from "../lib/types";

  // [ISO 639-1, ISO 639-2 (as Plex reports stream languages), name]
  const LANGUAGES: [string, string, string][] = [
    ["ar", "ara", "Arabic"],
    ["bg", "bul", "Bulgarian"],
    ["zh", "chi", "Chinese"],
    ["hr", "hrv", "Croatian"],
    ["cs", "cze", "Czech"],
    ["da", "dan", "Danish"],
    ["nl", "dut", "Dutch"],
    ["en", "eng", "English"],
    ["et", "est", "Estonian"],
    ["fi", "fin", "Finnish"],
    ["fr", "fre", "French"],
    ["de", "ger", "German"],
    ["el", "gre", "Greek"],
    ["he", "heb", "Hebrew"],
    ["hi", "hin", "Hindi"],
    ["hu", "hun", "Hungarian"],
    ["is", "ice", "Icelandic"],
    ["id", "ind", "Indonesian"],
    ["it", "ita", "Italian"],
    ["ja", "jpn", "Japanese"],
    ["ko", "kor", "Korean"],
    ["lv", "lav", "Latvian"],
    ["lt", "lit", "Lithuanian"],
    ["ms", "may", "Malay"],
    ["no", "nor", "Norwegian"],
    ["fa", "per", "Persian"],
    ["pl", "pol", "Polish"],
    ["pt", "por", "Portuguese"],
    ["ro", "rum", "Romanian"],
    ["ru", "rus", "Russian"],
    ["sr", "srp", "Serbian"],
    ["sk", "slo", "Slovak"],
    ["sl", "slv", "Slovenian"],
    ["es", "spa", "Spanish"],
    ["sv", "swe", "Swedish"],
    ["th", "tha", "Thai"],
    ["tr", "tur", "Turkish"],
    ["uk", "ukr", "Ukrainian"],
    ["vi", "vie", "Vietnamese"],
  ];
  // ISO 639-2/T variants Plex sometimes uses.
  const ALIASES: Record<string, string> = { zho: "zh", ces: "cs", nld: "nl", fra: "fr", deu: "de", ell: "el", isl: "is", msa: "ms", fas: "fa", ron: "ro", slk: "sk" };

  let item = $derived(app.subtitleTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let language = $state("en");
  let hearingImpaired = $state(false);
  let forced = $state(false);
  let results = $state<Stream[] | null>(null);
  let error = $state<string>();
  let busy = $state<string | null>(null);
  let generation = 0;

  function toIso1(code?: string) {
    if (!code) return null;
    const c = code.toLowerCase();
    return LANGUAGES.find(([a, b]) => a === c || b === c)?.[0] ?? ALIASES[c] ?? null;
  }

  function defaultLanguage(m: Metadata) {
    const part = m.Media[0]?.Part[0];
    if (!part) return "en";
    return toIso1(plex.selectedSubtitle(part)?.languageCode) ?? toIso1(plex.selectedAudio(part)?.languageCode) ?? "en";
  }

  function search() {
    const target = item;
    if (!target) return;
    const run = ++generation;
    const asked = guard;
    results = null;
    error = undefined;
    api.searchSubtitles(target.ratingKey ?? "", language, hearingImpaired, forced).then(
      (r) => run === generation && asked.valid() && (results = r),
      (e) => run === generation && asked.valid() && (error = message(e)),
    );
  }

  $effect(() => {
    const target = item;
    busy = null;
    if (!target) return;
    guard = captureScope();
    language = defaultLanguage(target);
    hearingImpaired = false;
    forced = false;
    untrack(search);
  });

  function close() {
    if (!busy) app.subtitleTarget = null;
  }

  async function fetchSub(s: Stream) {
    if (!item || !s.key || busy) return;
    if (!guard.valid()) { app.subtitleTarget = null; return; }
    busy = s.key;
    const ok = await act(api.downloadSubtitle(item.ratingKey ?? "", s.key), "Subtitle downloaded", true);
    busy = null;
    if (ok) app.subtitleTarget = null;
  }
</script>

{#if item}
  <dialog class="modal wide" use:modal={close} aria-labelledby="subtitle-title">
    <h2 id="subtitle-title">Search subtitles</h2>
    <p class="desc">{plex.fullTitle(item)}</p>

    <div class="mb-4 flex items-center gap-2">
      <NativeSelect.Root
        class="mr-1 min-w-40"
        aria-label="Language"
        value={language}
        onchange={(e) => {
          language = e.currentTarget.value;
          search();
        }}
      >
        {#each LANGUAGES as [code, , name] (code)}<NativeSelect.Option value={code}>{name}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
      <button
        class={["chip", { active: hearingImpaired }]}
        onclick={() => {
          hearingImpaired = !hearingImpaired;
          search();
        }}>Hearing impaired</button
      >
      <button
        class={["chip", { active: forced }]}
        onclick={() => {
          forced = !forced;
          search();
        }}>Forced only</button
      >
    </div>

    {#if error}
      <Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap select-text">{error}</Alert.Description></Alert.Root>
    {:else if results === null}
      <div class="text-muted-foreground flex items-center gap-2.5 py-2 text-sm">
        <Spinner />
        <span>Searching subtitle providers…</span>
      </div>
    {:else if results.length === 0}
      <Empty.Root class="border border-dashed p-6">
        <Empty.Header>
          <Empty.Title>No subtitles</Empty.Title>
          <Empty.Description>Nothing found for this language.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <div class="rows max-h-[52vh] overflow-auto">
        {#each results as s, i (s.key ?? i)}
          <div class="flex items-center gap-3 px-3 py-2">
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium" title={s.displayTitle ?? s.title}>{s.displayTitle ?? s.title ?? "Subtitle"}</div>
              <div class="text-muted-foreground truncate text-xs">
                {[s.providerTitle, s.title && s.title !== s.displayTitle ? s.title : null].filter(Boolean).join(" · ")}
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              {#if s.perfectMatch}<Badge variant="outline" class="text-tint border-tint/40 bg-tint/10">Perfect match</Badge>{/if}
              {#if s.hearingImpaired}<Badge variant="outline">SDH</Badge>{/if}
              {#if s.forced}<Badge variant="outline">Forced</Badge>{/if}
              {#if s.score != null}<span class="text-muted-foreground min-w-[26px] text-right text-xs tabular-nums">{s.score}</span>{/if}
              {#if s.downloaded}
                <span class="text-muted-foreground text-xs">Downloaded</span>
              {:else}
                <Button variant="outline" size="sm" disabled={!s.key || !!busy} onclick={() => fetchSub(s)}>
                  {#if busy && busy === s.key}<Spinner />{:else}<Download />{/if}Download
                </Button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <div class="foot"><Button variant="outline" disabled={!!busy} onclick={close}>Close</Button></div>
  </dialog>
{/if}
