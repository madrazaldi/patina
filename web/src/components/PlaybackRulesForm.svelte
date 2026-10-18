<script lang="ts">
  // The fields of one set of automatic playback preferences. Used for the
  // profile (Settings) and for a single show; a show's unset fields inherit the
  // profile's. The matching itself is in crates/core/src/prefs.rs.
  import * as Field from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { cn } from "$lib/utils";
  import type { ForcedMode, HdrMode, PlaybackRules, ResolutionPref, SubtitleMode } from "../lib/types";

  let {
    rules,
    unset = "Not set",
    onsave,
    disabled = false,
    class: className,
  }: {
    rules: PlaybackRules;
    /** Label of the "no preference" choice. */
    unset?: string;
    onsave: (rules: PlaybackRules) => void;
    disabled?: boolean;
    class?: string;
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

<div class={cn("bg-card/60 divide-y overflow-hidden rounded-xl border", className)}>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Audio languages</Field.Title>
      <Field.Description class="text-[13px]">In order of preference, e.g. “eng, ja”. Codes or names.</Field.Description>
    </Field.Content>
    <div class="w-52 flex-none">
      <Input
        placeholder={unset}
        {disabled}
        value={(rules.audioLanguages ?? []).join(", ")}
        onchange={(e) => set({ audioLanguages: parts(e.currentTarget.value).length ? parts(e.currentTarget.value) : null })}
      />
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Skip commentary</Field.Title>
      <Field.Description class="text-[13px]">Never pick commentary or audio-description tracks.</Field.Description>
    </Field.Content>
    <div class="w-52 flex-none">
      <NativeSelect.Root class="w-full" {disabled} value={rules.avoidCommentary == null ? NONE : String(rules.avoidCommentary)} onchange={(e) => set({ avoidCommentary: e.currentTarget.value === NONE ? null : e.currentTarget.value === "true" })}>
        <NativeSelect.Option value={NONE}>{unset}</NativeSelect.Option>
        <NativeSelect.Option value="true">Skip commentary</NativeSelect.Option>
        <NativeSelect.Option value="false">Allow commentary</NativeSelect.Option>
      </NativeSelect.Root>
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Subtitles</Field.Title>
      <Field.Description class="text-[13px]">When subtitles are turned on.</Field.Description>
    </Field.Content>
    <div class="w-52 flex-none">
      <NativeSelect.Root class="w-full" {disabled} value={rules.subtitles ?? NONE} onchange={(e) => set({ subtitles: pick<SubtitleMode>(e.currentTarget.value) })}>
        <NativeSelect.Option value={NONE}>{unset}</NativeSelect.Option>
        {#each SUBTITLES as [id, label] (id)}<NativeSelect.Option value={id}>{label}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Subtitle languages</Field.Title>
      <Field.Description class="text-[13px]">The audio languages when empty.</Field.Description>
    </Field.Content>
    <div class="w-52 flex-none">
      <Input
        placeholder={unset}
        {disabled}
        value={(rules.subtitleLanguages ?? []).join(", ")}
        onchange={(e) => set({ subtitleLanguages: parts(e.currentTarget.value).length ? parts(e.currentTarget.value) : null })}
      />
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Forced subtitles</Field.Title>
      <Field.Description class="text-[13px]">Which kind of subtitle track to pick.</Field.Description>
    </Field.Content>
    <div class="w-52 flex-none">
      <NativeSelect.Root class="w-full" {disabled} value={rules.forced ?? NONE} onchange={(e) => set({ forced: pick<ForcedMode>(e.currentTarget.value) })}>
        <NativeSelect.Option value={NONE}>{unset}</NativeSelect.Option>
        {#each FORCED as [id, label] (id)}<NativeSelect.Option value={id}>{label}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Version: resolution</Field.Title>
      <Field.Description class="text-[13px]">Which version to play when a title has several files.</Field.Description>
    </Field.Content>
    <div class="w-52 flex-none">
      <NativeSelect.Root class="w-full" {disabled} value={rules.resolution ?? NONE} onchange={(e) => set({ resolution: pick<ResolutionPref>(e.currentTarget.value) })}>
        <NativeSelect.Option value={NONE}>{unset}</NativeSelect.Option>
        {#each RESOLUTION as [id, label] (id)}<NativeSelect.Option value={id}>{label}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Version: video codec</Field.Title>
    </Field.Content>
    <div class="w-52 flex-none">
      <NativeSelect.Root class="w-full" {disabled} value={rules.codec ?? NONE} onchange={(e) => set({ codec: e.currentTarget.value === NONE ? null : e.currentTarget.value })}>
        <NativeSelect.Option value={NONE}>{unset}</NativeSelect.Option>
        {#each codecs as [id, label] (id)}<NativeSelect.Option value={id}>{label}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
    </div>
  </div>
  <div class="flex items-center justify-between gap-6 px-4 py-3.5">
    <Field.Content>
      <Field.Title>Version: HDR</Field.Title>
    </Field.Content>
    <div class="w-52 flex-none">
      <NativeSelect.Root class="w-full" {disabled} value={rules.hdr ?? NONE} onchange={(e) => set({ hdr: pick<HdrMode>(e.currentTarget.value) })}>
        <NativeSelect.Option value={NONE}>{unset}</NativeSelect.Option>
        {#each HDR as [id, label] (id)}<NativeSelect.Option value={id}>{label}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
    </div>
  </div>
</div>
