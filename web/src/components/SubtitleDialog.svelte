<script lang="ts">
  // Online subtitle search for a movie or episode, shown while `app.subtitleTarget` is set.
  import { untrack } from "svelte";
  import { Download } from "@lucide/svelte";
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { act, app, message } from "../lib/state.svelte";
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
    results = null;
    error = undefined;
    api.searchSubtitles(target.ratingKey ?? "", language, hearingImpaired, forced).then(
      (r) => run === generation && (results = r),
      (e) => run === generation && (error = message(e)),
    );
  }

  $effect(() => {
    const target = item;
    busy = null;
    if (!target) return;
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
    busy = s.key;
    const ok = await act(api.downloadSubtitle(item.ratingKey ?? "", s.key), "Subtitle downloaded", true);
    busy = null;
    if (ok) app.subtitleTarget = null;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && item && close()} />

{#if item}
  <div class="modal-backdrop" role="presentation" onclick={close}>
    <div class="modal wide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>Search subtitles</h2>
      <p class="desc">{plex.fullTitle(item)}</p>

      <div class="filters">
        <select
          class="select"
          value={language}
          onchange={(e) => {
            language = e.currentTarget.value;
            search();
          }}
        >
          {#each LANGUAGES as [code, , name] (code)}<option value={code}>{name}</option>{/each}
        </select>
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
        <div class="error-box">{error}</div>
      {:else if results === null}
        <div class="loading">
          <div class="spinner"></div>
          <span class="faint">Searching subtitle providers…</span>
        </div>
      {:else if results.length === 0}
        <p class="faint">Nothing found for this language.</p>
      {:else}
        <div class="rows results">
          {#each results as s, i (s.key ?? i)}
            <div class="list-row result">
              <div class="grow">
                <div class="t" title={s.displayTitle ?? s.title}>{s.displayTitle ?? s.title ?? "Subtitle"}</div>
                <div class="s">
                  {[s.providerTitle, s.title && s.title !== s.displayTitle ? s.title : null].filter(Boolean).join(" · ")}
                </div>
              </div>
              <div class="end">
                {#if s.perfectMatch}<span class="tag accent">Perfect match</span>{/if}
                {#if s.hearingImpaired}<span class="tag">SDH</span>{/if}
                {#if s.forced}<span class="tag">Forced</span>{/if}
                {#if s.score != null}<span class="score">{s.score}</span>{/if}
                {#if s.downloaded}
                  <span class="faint done">Downloaded</span>
                {:else}
                  <button class="btn sm" disabled={!s.key || !!busy} onclick={() => fetchSub(s)}>
                    {#if busy && busy === s.key}<span class="spinner small"></span>{:else}<Download size={13} />{/if}Download
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <div class="foot"><button class="btn quiet" disabled={!!busy} onclick={close}>Close</button></div>
    </div>
  </div>
{/if}

<style>
  .filters {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
  }
  .filters .select {
    min-width: 160px;
    margin-right: 4px;
  }
  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .results {
    max-height: 52vh;
    overflow: auto;
  }
  .result {
    grid-template-columns: 1fr auto;
  }
  .result .end {
    gap: 8px;
  }
  .score {
    min-width: 26px;
    text-align: right;
    font-size: 12px;
  }
  .done {
    font-size: 12px;
  }
  .spinner.small {
    width: 12px;
    height: 12px;
  }
</style>
