<script lang="ts">
  // The fields of one set of automatic playback preferences. Used for the
  // profile (Settings) and for a single show; a show's unset fields inherit the
  // profile's. The matching itself is in crates/core/src/prefs.rs.
  import type { ForcedMode, HdrMode, PlaybackRules, ResolutionPref, SubtitleMode } from "../lib/types";

  let {
    rules,
    unset = "Not set",
    onsave,
    disabled = false,
  }: {
    rules: PlaybackRules;
    /** Label of the "no preference" choice. */
    unset?: string;
    onsave: (rules: PlaybackRules) => void;
    disabled?: boolean;
  } = $props();

  const NONE = "";
  const parts = (text: string) =>
    text
      .split(/[,;]/)
      .map((s) => s.trim())
      .filter(Boolean);

  const set = (patch: Partial<PlaybackRules>) => onsave({ ...rules, ...patch });
  const pick = <T extends string>(v: string): T | null => (v === NONE ? null : (v as T));

  const SUBTITLES: [SubtitleMode, string][] = [
    ["keep", "Leave as Plex has them"],
    ["off", "Off"],
    ["always", "Always on"],
    ["foreign", "Only for foreign dialogue"],
  ];
  const FORCED: [ForcedMode, string][] = [
    ["auto", "Full subtitles (forced if nothing else)"],
    ["prefer", "Forced first, then full"],
    ["avoid", "Never forced"],
    ["only", "Only forced"],
  ];
  const RESOLUTION: [ResolutionPref, string][] = [
    ["any", "Any"],
    ["4k", "4K"],
    ["1080", "1080p"],
    ["720", "720p"],
    ["sd", "SD"],
  ];
  const CODECS: [string, string][] = [
    ["h264", "H.264"],
    ["hevc", "HEVC (H.265)"],
    ["av1", "AV1"],
    ["vp9", "VP9"],
  ];
  const HDR: [HdrMode, string][] = [
    ["any", "Any"],
    ["prefer", "Prefer HDR"],
    ["avoid", "Avoid HDR (SDR)"],
  ];

  let codecs = $derived(rules.codec && !CODECS.some(([id]) => id === rules.codec) ? [...CODECS, [rules.codec, rules.codec] as [string, string]] : CODECS);
</script>

<div class="rows">
  <div class="setting">
    <div class="label"><b>Audio languages</b><span>In order of preference, e.g. “eng, ja”. Codes or names.</span></div>
    <div class="control">
      <input
        class="input"
        style="width: 200px"
        placeholder={unset}
        {disabled}
        value={(rules.audioLanguages ?? []).join(", ")}
        onchange={(e) => set({ audioLanguages: parts(e.currentTarget.value).length ? parts(e.currentTarget.value) : null })}
      />
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Skip commentary</b><span>Never pick commentary or audio-description tracks.</span></div>
    <div class="control">
      <select
        class="select"
        style="width: 200px"
        {disabled}
        value={rules.avoidCommentary == null ? NONE : String(rules.avoidCommentary)}
        onchange={(e) => set({ avoidCommentary: e.currentTarget.value === NONE ? null : e.currentTarget.value === "true" })}
      >
        <option value={NONE}>{unset}</option>
        <option value="true">Skip commentary</option>
        <option value="false">Allow commentary</option>
      </select>
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Subtitles</b><span>When subtitles are turned on.</span></div>
    <div class="control">
      <select
        class="select"
        style="width: 200px"
        {disabled}
        value={rules.subtitles ?? NONE}
        onchange={(e) => set({ subtitles: pick<SubtitleMode>(e.currentTarget.value) })}
      >
        <option value={NONE}>{unset}</option>
        {#each SUBTITLES as [id, label] (id)}<option value={id}>{label}</option>{/each}
      </select>
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Subtitle languages</b><span>The audio languages when empty.</span></div>
    <div class="control">
      <input
        class="input"
        style="width: 200px"
        placeholder={unset}
        {disabled}
        value={(rules.subtitleLanguages ?? []).join(", ")}
        onchange={(e) => set({ subtitleLanguages: parts(e.currentTarget.value).length ? parts(e.currentTarget.value) : null })}
      />
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Forced subtitles</b><span>Which kind of subtitle track to pick.</span></div>
    <div class="control">
      <select
        class="select"
        style="width: 200px"
        {disabled}
        value={rules.forced ?? NONE}
        onchange={(e) => set({ forced: pick<ForcedMode>(e.currentTarget.value) })}
      >
        <option value={NONE}>{unset}</option>
        {#each FORCED as [id, label] (id)}<option value={id}>{label}</option>{/each}
      </select>
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Version: resolution</b><span>Which version to play when a title has several files.</span></div>
    <div class="control">
      <select
        class="select"
        style="width: 200px"
        {disabled}
        value={rules.resolution ?? NONE}
        onchange={(e) => set({ resolution: pick<ResolutionPref>(e.currentTarget.value) })}
      >
        <option value={NONE}>{unset}</option>
        {#each RESOLUTION as [id, label] (id)}<option value={id}>{label}</option>{/each}
      </select>
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Version: video codec</b></div>
    <div class="control">
      <select
        class="select"
        style="width: 200px"
        {disabled}
        value={rules.codec ?? NONE}
        onchange={(e) => set({ codec: e.currentTarget.value === NONE ? null : e.currentTarget.value })}
      >
        <option value={NONE}>{unset}</option>
        {#each codecs as [id, label] (id)}<option value={id}>{label}</option>{/each}
      </select>
    </div>
  </div>
  <div class="setting">
    <div class="label"><b>Version: HDR</b></div>
    <div class="control">
      <select
        class="select"
        style="width: 200px"
        {disabled}
        value={rules.hdr ?? NONE}
        onchange={(e) => set({ hdr: pick<HdrMode>(e.currentTarget.value) })}
      >
        <option value={NONE}>{unset}</option>
        {#each HDR as [id, label] (id)}<option value={id}>{label}</option>{/each}
      </select>
    </div>
  </div>
</div>
