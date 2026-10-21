<script lang="ts">
  // An album's songs in two columns: click to select, double-click to play,
  // drag to a playlist, right-click for the song menu.
  import { store, isMac } from "./store.svelte";
  import { duration } from "./format";
  import { songMenu, startSongDrag } from "./trackActions";
  import type { Track } from "./types";

  let { tracks, dark = false }: { tracks: Track[]; dark?: boolean } = $props();
  const discs = $derived(new Set(tracks.map((t) => t.discNr || 1)).size > 1);
  const half = $derived(tracks.length > 8 ? Math.ceil(tracks.length / 2) : tracks.length);

  function down(e: MouseEvent, t: Track) {
    if (isMac && e.ctrlKey) return; // a right-click
    startSongDrag(e, t, tracks);
    if (e.button === 0 && !(store.selected.has(t.id) && !e.shiftKey && !e.metaKey && !e.ctrlKey && store.selected.size > 1)) {
      store.clickTrack(t.id, tracks, e);
    }
  }
  function key(e: KeyboardEvent, i: number) {
    if (e.key === "Enter") void store.playList(tracks, i);
  }
</script>

<ol class="tracks" class:dark class:one={half === tracks.length} style:--rows={half}>
  {#each tracks as t, i (t.id)}
    <li
      class:sel={store.selected.has(t.id)}
      class:playing={store.nowPlaying === t.dbid}
      class:unavailable={t.unavailable}
      role="option"
      aria-selected={store.selected.has(t.id)}
      tabindex="0"
      onmousedown={(e) => down(e, t)}
      ondblclick={() => void store.playList(tracks, i)}
      oncontextmenu={(e) => songMenu(e, t, tracks)}
      onkeydown={(e) => key(e, i)}
    >
      <span class="nr">{store.nowPlaying === t.dbid ? "▶" : discs ? `${t.discNr || 1}-${t.trackNr || i + 1}` : t.trackNr || i + 1}</span>
      <span class="title">{t.title ?? ""}{#if t.compilation && t.artist}<span class="by">{" — "}{t.artist}</span>{/if}</span>
      <span class="time">{duration(t.lengthMs)}</span>
    </li>
  {/each}
</ol>

<style>
  .tracks {
    display: grid;
    grid-auto-flow: column;
    grid-template-rows: repeat(var(--rows), auto);
    grid-auto-columns: minmax(0, 1fr);
    column-gap: 28px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 12.5px;
    max-width: 880px;
  }
  .tracks.one {
    max-width: 460px;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 3px 8px;
    border-radius: 5px;
    cursor: default;
    user-select: none;
    -webkit-user-select: none;
  }
  li:hover {
    background: var(--zebra);
  }
  li.sel {
    background: var(--sel);
    color: var(--sel-ink);
  }
  li:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .nr {
    flex: none;
    min-width: 2.2em;
    text-align: right;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .playing .nr {
    color: var(--accent);
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .by {
    color: var(--muted);
  }
  .time {
    flex: none;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  li.sel .nr,
  li.sel .time,
  li.sel .by {
    color: inherit;
  }
  .unavailable {
    font-style: italic;
    opacity: 0.6;
  }
  .dark li:hover {
    background: rgb(255 255 255 / 0.07);
  }
</style>
