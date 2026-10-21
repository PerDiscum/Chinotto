<script lang="ts">
  import { store } from "./store.svelte";

  // Like iTunes: the playing song's cover, or the selected song's when you choose.
  let mode = $state<"playing" | "selected">("playing");
  const track = $derived(mode === "playing" && store.playingTrack ? store.playingTrack : store.selectedTracks[0]);
  const showingPlaying = $derived(mode === "playing" && !!store.playingTrack);
  let failed = $state(false);
  const src = $derived(store.coverUrl(track, 320));
  $effect(() => {
    void src;
    failed = false;
  });

  function contextmenu(e: MouseEvent) {
    e.preventDefault();
    const targets = store.selectedTracks.map((t) => t.dbid);
    const n = targets.length;
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Save Cover As…", disabled: !track?.art || failed, action: () => track && store.saveCover(track) },
        {
          label: n > 1 ? `Paste Cover to ${n} Selected Songs` : "Paste Cover to Selected Song",
          disabled: !n || !store.writable || !!store.busy || store.source.kind === "library",
          action: () => store.pasteCover(targets),
        },
      ],
    };
  }
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
  <div class="frame" role="img" aria-label="Cover" oncontextmenu={contextmenu}>
    {#if src && !failed}
      <img {src} alt="Cover of {track?.album ?? track?.title ?? 'the selected song'}" onerror={() => (failed = true)} />
    {:else}
      <p>{track ? (src && failed ? "Cover file missing" : "No artwork") : "Select a song to see its cover"}</p>
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
