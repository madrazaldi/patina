<script lang="ts">
  // Loading, empty and error placeholders.
  import { CircleAlert, TriangleAlert } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import { Spinner } from "$lib/components/ui/spinner";

  let {
    loading = false,
    error,
    title,
    text,
    fill = false, retry, stale = false,
  }: { loading?: boolean; error?: string; title?: string; text?: string; fill?: boolean; retry?: () => void; stale?: boolean } = $props();
</script>

<div class={["status", stale && error ? "mb-4" : "w-full"]} role="status">
  {#if loading}
    <div class={["flex items-center justify-center", fill ? "min-h-[60vh]" : "min-h-44"]}>
      <Spinner class="size-5 text-muted-foreground" role="presentation" aria-label="" />
    </div>
  {:else if error && stale}
    <Alert.Root role="none">
      <TriangleAlert />
      <Alert.Title>Showing saved results</Alert.Title>
      <Alert.Description class="whitespace-pre-wrap select-text">{error}</Alert.Description>
      {#if retry}
        <Alert.Action><Button variant="outline" size="xs" onclick={retry}>Retry</Button></Alert.Action>
      {/if}
    </Alert.Root>
  {:else if error}
    <Empty.Root class={["border", fill ? "min-h-[60vh]" : "min-h-44"]}>
      <Empty.Header>
        <Empty.Media variant="icon"><CircleAlert /></Empty.Media>
        <Empty.Title>Could not load this page</Empty.Title>
      </Empty.Header>
      <Empty.Content class="max-w-xl">
        <Alert.Root variant="destructive" role="none"><Alert.Description class="whitespace-pre-wrap select-text">{error}</Alert.Description></Alert.Root>
        {#if retry}<Button variant="outline" onclick={retry}>Retry</Button>{/if}
      </Empty.Content>
    </Empty.Root>
  {:else}
    <Empty.Root class={["border", fill ? "min-h-[60vh]" : "min-h-44"]}>
      <Empty.Header>
        {#if title}<Empty.Title class="text-base">{title}</Empty.Title>{/if}
        {#if text}<Empty.Description>{text}</Empty.Description>{/if}
      </Empty.Header>
    </Empty.Root>
  {/if}
</div>
