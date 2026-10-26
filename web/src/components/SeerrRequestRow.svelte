<script lang="ts">
  // One Seerr request, with what the user may do to it: approve or decline
  // (managers), retry a failed one, or withdraw it.
  import { Check, RotateCcw, Trash, X } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import Img from "./Img.svelte";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import { nav } from "../lib/router.svelte";
  import { availabilityLabel, requestLabel } from "../lib/seerr";
  import { act, app } from "../lib/state.svelte";
  import type { SeerrCan, SeerrRequest } from "../lib/types";

  let {
    request: r,
    can,
    onchange,
    linked = true,
  }: { request: SeerrRequest; can: SeerrCan | null; onchange: () => void; linked?: boolean } = $props();

  let busy = $state(false);
  let manage = $derived(can?.manage ?? false);
  let withdraw = $derived(manage || (r.mine && r.state === "pending"));

  const when = (iso: string | null) => (iso ? new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" }) : "");

  let details = $derived(
    [
      r.mediaType === "tv" ? (r.seasons.length ? `Season${r.seasons.length === 1 ? "" : "s"} ${r.seasons.join(", ")}` : "Show") : "Movie",
      r.is4k ? "4K" : null,
      r.requestedBy,
      when(r.createdAt),
    ]
      .filter(Boolean)
      .join(" · "),
  );
  // Once approved, how far the title got says more than the request itself.
  let shown = $derived(
    (r.state === "approved" || r.state === "completed") && (r.status === "processing" || r.status === "partial" || r.status === "available")
      ? availabilityLabel(r.status)!
      : requestLabel(r.state),
  );

  async function run(action: "approve" | "decline" | "retry" | "delete", done: string) {
    if (action === "delete" || action === "decline") {
      const ok = await app.confirm(action === "delete" ? `Delete the request for “${r.title}”?` : `Decline “${r.title}”?`, {
        text: action === "delete" ? "The request is removed from Seerr." : `${r.requestedBy} is told it was declined.`,
        action: action === "delete" ? "Delete" : "Decline",
        danger: true,
      });
      if (!ok) return;
    }
    busy = true;
    if (await act(api.seerrRequestAction(r.id, action), done)) onchange();
    busy = false;
  }

  const open = () => nav.push({ name: "seerr", mediaType: r.mediaType, tmdbId: r.tmdbId });
</script>

{#snippet content()}
  <div class="thumb">{#if r.poster}<Img src={img(r.poster, 96, 144)} />{/if}</div>
  <div class="grow">
    <div class="t">{r.title}{#if r.year}<span class="ml-1 text-muted-foreground">({r.year})</span>{/if}</div>
    <div class="s">{details}</div>
  </div>
  <div class="end">
    <Badge
      variant="outline"
      class={[
        r.state === "failed" || r.state === "declined" ? "border-destructive/40 text-destructive" : "",
        r.status === "available" && r.state !== "declined" ? "border-tint/40 bg-tint/10 text-tint" : "",
      ]}>{shown}</Badge
    >
    {#if manage && r.state === "pending"}
      <Button variant="ghost" size="icon-sm" title="Approve" disabled={busy} onclick={(e) => { e.stopPropagation(); run("approve", "Request approved"); }}><Check /></Button>
      <Button variant="ghost" size="icon-sm" title="Decline" disabled={busy} onclick={(e) => { e.stopPropagation(); run("decline", "Request declined"); }}><X /></Button>
    {/if}
    {#if manage && r.state === "failed"}
      <Button variant="ghost" size="icon-sm" title="Retry" disabled={busy} onclick={(e) => { e.stopPropagation(); run("retry", "Sent again"); }}><RotateCcw /></Button>
    {/if}
    {#if withdraw}
      <Button variant="ghost" size="icon-sm" title="Delete request" disabled={busy} onclick={(e) => { e.stopPropagation(); run("delete", "Request deleted"); }}><Trash /></Button>
    {/if}
  </div>
{/snippet}

{#if linked}
  <div class="list-row clickable" role="button" tabindex="0" onclick={open} onkeydown={(e) => e.target === e.currentTarget && e.key === "Enter" && open()}>
    {@render content()}
  </div>
{:else}
  <div class="list-row">{@render content()}</div>
{/if}
