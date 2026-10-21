<script lang="ts">
  import { store } from "./store.svelte";
  import { count, duration, longDuration } from "./format";
  import type { Track } from "./types";

  let { visible, visibleMs }: { visible: Track[]; visibleMs: number } = $props();
  const visibleCount = $derived(visible.length);

  const song = $derived(store.playingTrack);
  const playing = $derived(store.player.state === "playing" && !!store.nowPlaying);
  const total = $derived(store.player.durationMs ?? song?.lengthMs ?? 0);
  // While the seek knob is being dragged, show where it is rather than the player's position.
  let dragging = $state<number | null>(null);
  const shown = $derived(dragging ?? store.player.positionMs);

  const openingName = $derived(
    store.opening ? (store.devices.find((d) => d.path === store.opening)?.volume ?? "iPod") : null,
  );
  const sourceName = $derived.by(() => {
    const s = store.source;
    if (s.kind === "device") return store.ipod?.name ?? "";
    if (s.kind === "library") return `${store.libView?.view.name ?? "Library"} — Music`;
    if (s.kind === "playlist") {
      const p = store.ipod?.playlists.find((x) => x.id === s.id);
      return p?.master ? `${store.ipod?.name} — Music` : (p?.name ?? "");
    }
    return "";
  });
</script>

<header class="toolbar" data-tauri-drag-region>
  <div class="controls">
    <div class="transport" role="group" aria-label="Playback">
      <button aria-label="Previous" title="Previous" disabled={!store.nowPlaying} onclick={() => store.previous()}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 5h2v14H6zM20 5v14L9 12z" /></svg>
      </button>
      <button class="main" aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause (Space) · Stop (⌘.)" : "Play (Space)"} disabled={!store.index} onclick={() => store.togglePlay(visible)}>
        {#if playing}
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h4v14H7zM13 5h4v14h-4z" /></svg>
        {:else}
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>
        {/if}
      </button>
      <button aria-label="Next" title="Next" disabled={!store.hasNext} onclick={() => store.next()}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M16 5h2v14h-2zM4 5v14l11-7z" /></svg>
      </button>
    </div>
    <label class="volume" title="Volume">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" /></svg>
      <input type="range" min="0" max="1" step="0.01" aria-label="Volume" value={store.volume} oninput={(e) => store.setVolume(Number(e.currentTarget.value))} />
    </label>
  </div>

  <div class="label" role="status" aria-live="polite">
    {#if store.busy}
      <strong>{store.busy}</strong>
      {#if store.progress && store.progress.total}
        <span class="bar" role="progressbar" aria-valuemin="0" aria-valuemax={store.progress.total} aria-valuenow={store.progress.done}>
          <span style:width="{(store.progress.done / store.progress.total) * 100}%"></span>
        </span>
      {:else}
        <span class="sub">Don't unplug the iPod</span>
      {/if}
    {:else if openingName}
      <strong>Reading {openingName}…</strong>
      <span class="sub">Loading songs and playlists</span>
    {:else if store.ejecting}
      <strong>Ejecting…</strong>
      <span class="sub">Don't unplug yet</span>
    {:else if song}
      <strong>{song.title ?? "Untitled"}</strong>
      <span class="sub">{[song.artist, song.album].filter(Boolean).join(" — ")}</span>
      <div class="seek">
        <span class="t">{duration(shown)}</span>
        <input
          type="range"
          min="0"
          max={Math.max(total, 1)}
          step="250"
          value={shown}
          aria-label="Position in song"
          aria-valuetext="{duration(shown)} of {duration(total)}"
          oninput={(e) => (dragging = Number(e.currentTarget.value))}
          onchange={(e) => {
            void store.seek(Number(e.currentTarget.value));
            dragging = null;
          }}
        />
        <span class="t">-{duration(Math.max(0, total - shown))}</span>
      </div>
    {:else if store.source.kind === "library" && store.scanProgress[store.source.lib]?.total}
      {@const p = store.scanProgress[store.source.lib]}
      <strong>Scanning {store.libView?.view.name}</strong>
      <span class="sub">{count(p.done)} of {count(p.total)} new or changed files{p.current ? `: ${p.current}` : ""}</span>
    {:else if store.ipod || store.source.kind === "library"}
      <strong>{sourceName}</strong>
      <span class="sub">
        {#if store.source.kind === "device" && store.ipod}
          {count(store.ipod.tracks.length)} songs on this iPod
        {:else}
          {count(visibleCount)} {visibleCount === 1 ? "song" : "songs"}, {longDuration(visibleMs)}
        {/if}
      </span>
    {:else}
      <strong>Chinotto</strong>
      <span class="sub">No iPod connected</span>
    {/if}
  </div>

  <div class="search">
    <input
      type="search"
      placeholder="Search"
      aria-label="Search songs"
      bind:value={store.search}
      oninput={() => store.searchStarted()}
      disabled={!store.ipod && store.source.kind !== "library"}
      spellcheck="false"
      autocomplete="off"
    />
  </div>
</header>

<style>
  .toolbar {
    grid-area: toolbar;
    display: grid;
    grid-template-columns: 1fr minmax(320px, 480px) 1fr;
    align-items: center;
    gap: 16px;
    padding: 8px 14px;
    background: var(--chrome);
    border-bottom: 1px solid var(--chrome-edge);
  }
  /* The one loud element: an amber bottle label. */
  .label {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 50px;
    padding: 4px 16px;
    border-radius: 9px;
    background: var(--label-bg);
    color: var(--label-ink);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.06),
      inset 0 0 0 1px rgb(0 0 0 / 0.5),
      0 0 0 1px var(--label-rim);
    font-family: var(--rounded);
    text-align: center;
    overflow: hidden;
  }
  .label strong {
    font-weight: 600;
    font-size: 13px;
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .label .sub {
    font-size: 11px;
    color: var(--label-dim);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    display: block;
    width: 70%;
    height: 4px;
    margin-top: 4px;
    border-radius: 2px;
    background: rgb(243 167 59 / 0.2);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--label-ink);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .transport button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--field-edge);
    border-radius: 50%;
    background: var(--field);
    color: var(--ink);
  }
  .transport button.main {
    width: 38px;
    height: 38px;
  }
  .transport svg {
    width: 14px;
    height: 14px;
    fill: currentColor;
  }
  .transport .main svg {
    width: 18px;
    height: 18px;
  }
  .transport button:disabled {
    opacity: 0.4;
  }
  .transport button:focus-visible,
  .volume input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  .volume svg {
    flex: none;
    width: 14px;
    height: 14px;
    fill: currentColor;
  }
  .volume input {
    width: 90px;
    accent-color: var(--accent);
  }
  .seek {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin-top: 2px;
  }
  .seek .t {
    flex: none;
    min-width: 38px;
    font-size: 10px;
    color: var(--label-dim);
    font-variant-numeric: tabular-nums;
  }
  .seek .t:first-child {
    text-align: right;
  }
  .seek .t:last-child {
    text-align: left;
  }
  .seek input {
    flex: 1;
    min-width: 0;
    height: 12px;
    margin: 0;
    background: transparent;
    -webkit-appearance: none;
    appearance: none;
  }
  .seek input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: rgb(243 167 59 / 0.25);
  }
  .seek input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 10px;
    height: 10px;
    margin-top: -3px;
    border-radius: 50%;
    background: var(--label-ink);
  }
  .seek input:focus-visible {
    outline: 1px solid var(--label-ink);
    outline-offset: 2px;
  }
  .search {
    justify-self: end;
  }
  .search input {
    width: 200px;
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid var(--field-edge);
    background: var(--field);
    color: var(--ink);
    font: inherit;
  }
  .search input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
