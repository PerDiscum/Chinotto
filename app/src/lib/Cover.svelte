<script lang="ts">
  import { store } from "./store.svelte";
  import type { Track } from "./types";

  let { track, px = 300, alt = "" }: { track: Track | null; px?: number; alt?: string } = $props();
  const src = $derived(store.coverUrl(track, px));
  let failed = $state(false);
  $effect(() => {
    void src;
    failed = false;
  });
</script>

{#if src && !failed}
  <img class="cover" {src} {alt} loading="lazy" decoding="async" draggable="false" onerror={() => (failed = true)} />
{:else}
  <div class="cover none" role="img" aria-label={alt || "No artwork"}>
    <svg viewBox="0 0 16 16" width="34%" height="34%" fill="none" stroke="currentColor" stroke-width="1.1" aria-hidden="true">
      <path d="M6 12.5V3.5l7-1.5v9" /><circle cx="4.3" cy="12.5" r="1.7" /><circle cx="11.3" cy="11" r="1.7" />
    </svg>
  </div>
{/if}

<style>
  .cover {
    display: block;
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    border-radius: 6px;
    background: var(--zebra);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.18);
  }
  .none {
    display: grid;
    place-items: center;
    color: var(--muted);
    background: linear-gradient(160deg, var(--zebra), var(--header));
  }
</style>
