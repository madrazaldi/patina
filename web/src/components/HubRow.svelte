<script lang="ts">
  // A Plex hub (Continue Watching, Recently Added, ...) rendered as a row.
  import Row from "./Row.svelte";
  import { browseRoute } from "../lib/router.svelte";
  import type { Hub } from "../lib/types";

  let { hub }: { hub: Hub } = $props();

  let ident = $derived((hub.hubIdentifier ?? "").toLowerCase());
  let continueStyle = $derived(["continue", "ondeck", "inprogress"].some((s) => ident.includes(s)));
  let landscape = $derived(continueStyle || hub.type === "clip" || hub.type === "episode");
  let title = $derived(hub.title ?? "");
  let more = $derived(hub.more && hub.key ? browseRoute(hub.key, title) : undefined);
</script>

<Row {title} items={hub.Metadata} shape={landscape ? "landscape" : undefined} useArt={continueStyle} {more} />
