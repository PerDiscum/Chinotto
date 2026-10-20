<script lang="ts">
  import { store } from "./store.svelte";
  import { artUrl } from "./api";

  const track = $derived(store.selectedTracks[0]);
  let failed = $state(false);
  const src = $derived(store.ipod && track?.art ? artUrl(store.ipod.generation, track.art, 320) : null);
  $effect(() => {
    void src;
    failed = false;
  });
</script>

<figure class="art" aria-label="Artwork of the selected song">
  <figcaption>{track ? "Selected song" : "Nothing selected"}</figcaption>
  <div class="frame">
    {#if src && !failed}
      <img {src} alt="Cover of {track?.album ?? track?.title ?? 'the selected song'}" onerror={() => (failed = true)} />
    {:else}
      <p>{track ? (track.art && failed ? "Cover file missing" : "No artwork") : "Select a song to see its cover"}</p>
    {/if}
  </div>
</figure>

<style>
  .art {
    flex: none;
    margin: 0;
    border-top: 1px solid var(--side-edge);
  }
  figcaption {
    padding: 5px 12px;
    font-size: 11px;
    font-weight: 600;
    color: var(--side-muted);
  }
  .frame {
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    background: var(--side-deep);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  p {
    margin: 0;
    padding: 0 16px;
    text-align: center;
    font-size: 12px;
    color: var(--side-muted);
  }
</style>
