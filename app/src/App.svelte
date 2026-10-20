<script lang="ts">
  import { onMount } from "svelte";
  import { store } from "./lib/store.svelte";
  import { browse } from "./lib/library";
  import { count, longDuration, bytes } from "./lib/format";
  import Toolbar from "./lib/Toolbar.svelte";
  import SourceList from "./lib/SourceList.svelte";
  import ColumnBrowser from "./lib/ColumnBrowser.svelte";
  import SongList from "./lib/SongList.svelte";
  import DevicePage from "./lib/DevicePage.svelte";
  import ArtworkPane from "./lib/ArtworkPane.svelte";
  import Welcome from "./lib/Welcome.svelte";
  import Icon from "./lib/Icon.svelte";

  onMount(() => {
    void store.init();
  });

  // Source → search → column browser → sort.
  const searched = $derived(store.index ? store.index.search(store.sourceTracks, store.search) : []);
  const browsed = $derived(
    store.showBrowser ? browse(searched, { genre: store.genre, artist: store.artist, album: store.album }) : null,
  );
  const filtered = $derived(browsed ? browsed.tracks : searched);
  const visible = $derived(store.index ? store.index.sort(filtered, store.sortKey, store.sortDir) : []);
  const totals = $derived.by(() => {
    let ms = 0, size = 0;
    for (const t of visible) { ms += t.lengthMs; size += t.size; }
    return { ms, size };
  });
</script>

<div class="app">
  <Toolbar visibleCount={visible.length} visibleMs={totals.ms} />

  <aside class="side">
    <SourceList />
    {#if store.showArtwork && store.ipod}<ArtworkPane />{/if}
    <div class="side-tools">
      <button title="Open iPod folder…" aria-label="Open iPod folder" onclick={() => store.openFolder()}><Icon name="folder" /></button>
      <button title={store.showBrowser ? "Hide column browser" : "Show column browser"} aria-pressed={store.showBrowser} onclick={() => (store.showBrowser = !store.showBrowser)}><Icon name="columns" /></button>
      <button title={store.showArtwork ? "Hide artwork" : "Show artwork"} aria-pressed={store.showArtwork} onclick={() => (store.showArtwork = !store.showArtwork)}><Icon name="artwork" /></button>
    </div>
  </aside>

  <main class="main">
    {#if store.error}
      <div class="error" role="alert">
        <span>{store.error}</span>
        <button onclick={() => (store.error = null)}>Dismiss</button>
      </div>
    {/if}

    {#if !store.ipod}
      <Welcome />
    {:else if store.source.kind === "device"}
      <DevicePage />
    {:else}
      {#if browsed}<ColumnBrowser columns={browsed.columns} />{/if}
      <SongList tracks={visible} />
    {/if}
  </main>

  <footer class="status">
    {#if store.ipod && store.source.kind === "playlist"}
      {count(visible.length)} {visible.length === 1 ? "song" : "songs"}, {longDuration(totals.ms)}, {bytes(totals.size)}{#if store.selected.size > 1}; {count(store.selected.size)} selected{/if}
    {/if}
  </footer>
</div>

<style>
  .app {
    display: grid;
    grid-template-areas: "toolbar toolbar" "side main" "side status";
    grid-template-columns: 220px 1fr;
    grid-template-rows: auto 1fr 22px;
    height: 100vh;
  }
  .side {
    grid-area: side;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--side);
    color: var(--side-ink);
    border-right: 1px solid var(--side-edge);
  }
  .side-tools {
    display: flex;
    gap: 2px;
    padding: 4px 6px;
    border-top: 1px solid var(--side-edge);
  }
  .side-tools button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 22px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--side-icon);
  }
  .side-tools button:hover {
    background: rgb(255 255 255 / 0.08);
    color: var(--side-ink);
  }
  .side-tools button[aria-pressed="true"] {
    color: var(--accent-bright);
  }
  .side-tools button:focus-visible {
    outline: 2px solid var(--accent-bright);
    outline-offset: -2px;
  }
  .main {
    grid-area: main;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .status {
    grid-area: status;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    color: var(--muted);
    background: var(--chrome);
    border-top: 1px solid var(--chrome-edge);
    font-variant-numeric: tabular-nums;
  }
  .error {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 12px;
    border-bottom: 1px solid var(--edge);
  }
  .error span {
    flex: 1;
  }
  .error button {
    border: 1px solid currentColor;
    border-radius: 5px;
    background: none;
    color: inherit;
    font: inherit;
    padding: 2px 10px;
  }
</style>
