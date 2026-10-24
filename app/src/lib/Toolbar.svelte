<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import { count, duration, longDuration } from "./format";
  import type { Track } from "./types";

  import { shownSongs } from "./commands.svelte";
  let { visible, visibleMs }: { visible: Track[]; visibleMs: number } = $props();
  const visibleCount = $derived(visible.length);

  const song = $derived(store.playingTrack);
  const playing = $derived(store.player.state === "playing" && !!store.nowPlaying);
  const total = $derived(store.player.durationMs ?? song?.lengthMs ?? 0);
  // While the seek knob is being dragged, show where it is rather than the player's position.
  let dragging = $state<number | null>(null);
  const shown = $derived(dragging ?? store.player.positionMs);

  const openingName = $derived(
    store.opening ? (store.devices.find((d) => d.path === store.opening)?.volume ?? t("device")) : null,
  );
  const sourceName = $derived.by(() => {
    const s = store.source;
    if (s.kind === "device") return store.ipod?.name ?? "";
    if (s.kind === "scratch") return t("Scratch");
    if (s.kind === "library") {
      const pl = store.libView?.view.playlists.find((p) => p.id === s.id);
      return pl && !pl.master ? pl.name : (store.libView?.view.name ?? t("Library"));
    }
    if (s.kind === "playlist") {
      const p = store.ipod?.playlists.find((x) => x.id === s.id);
      return p?.master ? t("{name} — Music", { name: store.ipod?.name ?? "" }) : (p?.name ?? "");
    }
    return "";
  });
</script>

<header class="toolbar" data-tauri-drag-region>
  <div class="controls">
    <div class="transport" role="group" aria-label={t("Playback")}>
      <button aria-label={t("Previous")} title={t("Previous")} disabled={!store.nowPlaying} onclick={() => store.previous()}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 5h2v14H6zM20 5v14L9 12z" /></svg>
      </button>
      <button class="main" aria-label={playing ? t("Pause") : t("Play")} title={playing ? t("Pause (Space) · Stop (⌘.)") : t("Play (Space)")} disabled={!store.index} onclick={() => store.togglePlay(shownSongs())}>
        {#if playing}
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h4v14H7zM13 5h4v14h-4z" /></svg>
        {:else}
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>
        {/if}
      </button>
      <button aria-label={t("Next")} title={t("Next")} disabled={!store.hasNext} onclick={() => store.next()}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M16 5h2v14h-2zM4 5v14l11-7z" /></svg>
      </button>
    </div>
    <label class="volume" title={t("Volume")}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" /></svg>
      <input type="range" min="0" max="1" step="0.01" aria-label={t("Volume")} value={store.volume} oninput={(e) => store.setVolume(Number(e.currentTarget.value))} />
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
        <span class="sub">{t("Don't unplug the device")}</span>
      {/if}
    {:else if openingName}
      <strong>{t("Reading {name}…", { name: openingName ?? "" })}</strong>
      <span class="sub">{t("Loading songs and playlists")}</span>
    {:else if store.ejecting}
      <strong>{t("Ejecting…")}</strong>
      <span class="sub">{t("Don't unplug yet")}</span>
    {:else if store.source.kind === "disc"}
      <strong class="mode">{t("Disc Rip Mode")}</strong>
    {:else if store.source.kind === "burn"}
      <strong class="mode">{t("Burn Image Mode")}</strong>
    {:else if song}
      <strong>{song.title ?? t("Untitled")}</strong>
      <span class="sub">{[song.artist, song.album].filter(Boolean).join(" — ")}</span>
      <div class="seek">
        <span class="t">{duration(shown)}</span>
        <input
          type="range"
          min="0"
          max={Math.max(total, 1)}
          step="250"
          value={shown}
          aria-label={t("Position in song")}
          aria-valuetext={t("{a} of {b}", { a: duration(shown), b: duration(total) })}
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
      <strong>{t("Scanning {name}", { name: store.libView?.view.name ?? "" })}</strong>
      <span class="sub">{t("{done} of {total} new or changed files", { done: p.done, total: p.total })}{p.current ? `: ${p.current}` : ""}</span>
    {:else if store.source.kind === "player"}
      {@const path = store.source.path}
      {@const d = store.devices.find((x) => x.path === path)}
      <strong>{d?.volume ?? ""}</strong>
      <span class="sub">{tn(store.libViews[d?.library ?? ""]?.view.tracks.length ?? 0, "{n} song on this player", "{n} songs on this player")}</span>
    {:else if store.source.kind === "openmg"}
      {@const path = store.source.path}
      {@const d = store.devices.find((x) => x.path === path)}
      <strong>{d?.volume ?? t("Sony Walkman")}</strong>
      <span class="sub">{t("OpenMG device")}</span>
    {:else if store.ipod || store.source.kind === "library" || store.source.kind === "scratch"}
      <strong>{sourceName}</strong>
      <span class="sub">
        {#if store.source.kind === "device" && store.ipod}
          {tn(store.ipod.tracks.length, "{n} song on this iPod", "{n} songs on this iPod")}
        {:else}
          {tn(visibleCount, "{n} song", "{n} songs")}, {longDuration(visibleMs)}
        {/if}
      </span>
    {:else}
      <strong>{t("Chinotto")}</strong>
      <span class="sub">{t("No device connected.")}</span>
    {/if}
  </div>

  <div class="search">
    <input
      type="search"
      placeholder={t("Search")}
      aria-label={t("Search songs")}
      bind:value={store.search}
      oninput={() => store.searchStarted()}
      disabled={!store.index}
      spellcheck="false"
      autocomplete="off"
    />
  </div>
</header>

<style>
  .mode {
    font-family: "Handjet", var(--ui);
    font-weight: 600;
    font-size: 42px;
    line-height: 1;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
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
