<script lang="ts">
  import { store } from "./store.svelte";
  import { artUrl } from "./api";

  // Like iTunes: the playing song's cover, or the selected song's when you choose.
  let mode = $state<"playing" | "selected">("playing");
  const track = $derived(mode === "playing" && store.playingTrack ? store.playingTrack : store.selectedTracks[0]);
  const showingPlaying = $derived(mode === "playing" && !!store.playingTrack);
  let failed = $state(false);
  const src = $derived(store.ipod && track?.art ? artUrl(store.ipod.generation, track.art, 320) : null);
  $effect(() => {
    void src;
    failed = false;
  });
</script>

<figure class="art" aria-label="Artwork of the selected song">
  <figcaption>
    {#if store.playingTrack}
      <button onclick={() => (mode = mode === "playing" ? "selected" : "playing")} title="Switch between the playing and the selected song">
        {showingPlaying ? "Now playing" : "Selected song"} <span aria-hidden="true">⇄</span>
      </button>
    {:else}
      {track ? "Selected song" : "Nothing selected"}
    {/if}
  </figcaption>
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
  figcaption button {
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
  }
  figcaption button:focus-visible {
    outline: 2px solid var(--accent-bright);
    outline-offset: 2px;
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
