<script lang="ts">
  import { store, isMac } from "./store.svelte";
  import { duration, stars, count } from "./format";
  import type { SortKey, Track } from "./types";

  let { tracks }: { tracks: Track[] } = $props();

  const ROW = 20;
  const OVERSCAN = 8;
  const columns: { key: SortKey; label: string; cls: string }[] = [
    { key: "title", label: "Name", cls: "c-title" },
    { key: "time", label: "Time", cls: "c-time" },
    { key: "artist", label: "Artist", cls: "c-artist" },
    { key: "album", label: "Album", cls: "c-album" },
    { key: "genre", label: "Genre", cls: "c-genre" },
    { key: "year", label: "Year", cls: "c-year" },
    { key: "rating", label: "Rating", cls: "c-rating" },
    { key: "plays", label: "Plays", cls: "c-plays" },
  ];

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(400);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(tracks.length, Math.ceil((scrollTop + viewport) / ROW) + OVERSCAN));
  const slice = $derived(tracks.slice(first, last));

  // Back to the top when the list itself changes (new playlist, search, browser pick).
  $effect(() => {
    void tracks;
    if (scroller) scroller.scrollTop = 0;
  });

  function keydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
      e.preventDefault();
      store.selectAll(tracks);
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "i") {
      e.preventDefault();
      if (store.selected.size) store.dialog = { kind: "info" };
      return;
    }
    if (e.key === "Enter" && store.selected.size) {
      e.preventDefault();
      playSelected();
      return;
    }
    if ((e.key === "Delete" || e.key === "Backspace") && store.selected.size) {
      e.preventDefault();
      store.removeSelected();
      return;
    }
    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : e.key === "PageDown" ? 20 : e.key === "PageUp" ? -20 : 0;
    if (!step) return;
    e.preventDefault();
    const i = store.moveSelection(tracks, step, e.shiftKey);
    if (i === null || !scroller) return;
    const top = i * ROW;
    const h = scroller.clientHeight - ROW; // minus sticky header
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW > scroller.scrollTop + h) scroller.scrollTop = top + ROW - h;
  }

  const dash = (s?: string) => s ?? "";

  /** Play from the first selected song, continuing down the list. */
  function playSelected() {
    const i = tracks.findIndex((x) => store.selected.has(x.id));
    if (i >= 0) void store.playList(tracks, i);
  }

  function contextmenu(e: MouseEvent, t: Track) {
    e.preventDefault();
    // Right-clicking outside the selection selects that song.
    if (!store.selected.has(t.id)) store.clickTrack(t.id, tracks, { shiftKey: false, metaKey: false, ctrlKey: false });
    const n = store.selected.size;
    const songs = n === 1 ? "Song" : `${n} Songs`;
    const ro = !store.writable;
    const pl = store.editablePlaylist;
    const targets = (store.ipod?.playlists ?? []).filter((p) => !p.master && !p.smart && p.id !== pl?.id);
    const dbids = store.selectedTracks.map((x) => x.dbid);
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Play", action: playSelected },
        { label: "Get Info", action: () => (store.dialog = { kind: "info" }) },
        { label: "", separator: true },
        {
          label: "Add to Playlist",
          disabled: ro,
          submenu: targets.map((p) => ({ label: p.name, action: () => store.addToPlaylist(p.id, dbids) })),
        },
        { label: "New Playlist from Selection", disabled: ro, action: () => store.newPlaylist(true) },
        { label: "", separator: true },
        ...(pl ? [{ label: `Remove from “${pl.name}”`, disabled: ro, action: () => store.removeFromPlaylist(pl.id, [...store.selected]) }] : []),
        {
          label: `Delete ${songs} from iPod…`,
          danger: true,
          disabled: ro,
          action: () => (store.dialog = { kind: "delete-songs", ids: dbids }),
        },
      ],
    };
  }

  /** Press on a song and move: drag the selection onto a playlist in the source list. */
  function pressForDrag(e: MouseEvent, t: Track) {
    if (e.button !== 0 || !store.writable) return;
    const x0 = e.clientX, y0 = e.clientY;
    const move = (m: MouseEvent) => {
      if (!store.songDrag) {
        if (Math.hypot(m.clientX - x0, m.clientY - y0) < 6) return;
        if (!store.selected.has(t.id)) store.clickTrack(t.id, tracks, { shiftKey: false, metaKey: false, ctrlKey: false });
        store.songDrag = { dbids: store.selectedTracks.map((x) => x.dbid), x: m.clientX, y: m.clientY, target: null };
      }
      const over = document.elementFromPoint(m.clientX, m.clientY)?.closest<HTMLElement>("[data-drop-playlist]");
      store.songDrag = { ...store.songDrag, x: m.clientX, y: m.clientY, target: over?.dataset.dropPlaylist ?? null };
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      const d = store.songDrag;
      store.songDrag = null;
      if (d?.target) void store.addToPlaylist(d.target, d.dbids);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }
</script>

<div
  class="songs"
  role="grid"
  tabindex="0"
  aria-label="Songs"
  aria-rowcount={tracks.length}
  aria-multiselectable="true"
  bind:this={scroller}
  bind:clientHeight={viewport}
  onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
  onkeydown={keydown}
>
  <div class="head row" role="row">
    {#each columns as c (c.key)}
      <button
        class="cell {c.cls}"
        class:sorted={store.sortKey === c.key}
        role="columnheader"
        aria-sort={store.sortKey === c.key ? (store.sortDir === 1 ? "ascending" : "descending") : "none"}
        onclick={() => store.sortBy(c.key)}
      >
        {c.label}
        {#if store.sortKey === c.key}<span class="arrow">{store.sortDir === 1 ? "▲" : "▼"}</span>{/if}
      </button>
    {/each}
  </div>

  <div class="body" style:height="{tracks.length * ROW}px">
    {#each slice as t, i (t.id)}
      {@const n = first + i}
      <!-- Keyboard use is handled by the list itself (arrows, Delete, Cmd/Ctrl+I). -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="row"
        class:odd={n % 2 === 1}
        class:sel={store.selected.has(t.id)}
        role="row"
        tabindex="-1"
        aria-rowindex={n + 1}
        aria-selected={store.selected.has(t.id)}
        style:transform="translateY({n * ROW}px)"
        onmousedown={(e) => {
          // Keep a multi-selection when starting a drag or right-clicking inside it.
          if (isMac && e.ctrlKey) return; // that's a right-click
          pressForDrag(e, t);
          if (e.button === 0 && !(store.selected.has(t.id) && !e.shiftKey && !e.metaKey && !e.ctrlKey && store.selected.size > 1)) store.clickTrack(t.id, tracks, e);
        }}
        onclick={(e) => {
          if (store.selected.size > 1 && store.selected.has(t.id) && !e.shiftKey && !e.metaKey && !e.ctrlKey) store.clickTrack(t.id, tracks, e);
        }}
        ondblclick={() => void store.playList(tracks, n)}
        oncontextmenu={(e) => contextmenu(e, t)}

      >
        <span class="cell c-title">
          {#if store.nowPlaying === t.dbid}<span class="playing" aria-label={store.player.state === "playing" ? "Playing" : "Paused"}>{store.player.state === "playing" ? "▶" : "❙❙"}</span>{/if}{dash(t.title)}
        </span>
        <span class="cell c-time num">{duration(t.lengthMs)}</span>
        <span class="cell c-artist">{dash(t.artist)}</span>
        <span class="cell c-album">{dash(t.album)}</span>
        <span class="cell c-genre">{dash(t.genre)}</span>
        <span class="cell c-year num">{t.year || ""}</span>
        <span class="cell c-rating" aria-label="{t.rating} stars">{stars(t.rating)}</span>
        <span class="cell c-plays num">{t.playCount ? count(t.playCount) : ""}</span>
      </div>
    {/each}
  </div>

  {#if tracks.length === 0}
    <p class="empty">
      {#if store.search}No songs match “{store.search}”.{:else}This playlist is empty.{/if}
    </p>
  {/if}
</div>

<style>
  .songs {
    position: relative;
    flex: 1;
    overflow: auto;
    background: var(--bg);
    font-size: 12px;
    outline: none;
    --cols: minmax(180px, 2.4fr) 52px minmax(120px, 1.5fr) minmax(120px, 1.5fr) minmax(80px, 0.9fr) 46px 76px 50px;
  }
  .songs:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .row {
    display: grid;
    grid-template-columns: var(--cols);
    height: 20px;
    align-items: center;
    min-width: 780px;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--header);
    border-bottom: 1px solid var(--edge);
  }
  .head .cell {
    height: 100%;
    border: 0;
    border-right: 1px solid var(--edge);
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 11px;
    font-weight: 600;
    text-align: left;
    cursor: default;
  }
  .head .cell.sorted {
    color: var(--ink);
    background: var(--header-sorted);
  }
  .head .cell:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .arrow {
    float: right;
    font-size: 8px;
    line-height: 20px;
  }
  .body {
    position: relative;
  }
  .body .row {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    color: var(--ink);
  }
  .body .row.odd {
    background: var(--zebra);
  }
  .body .row.sel {
    background: var(--sel-quiet);
  }
  .songs:focus-within .body .row.sel {
    background: var(--sel);
    color: var(--sel-ink);
  }
  .cell {
    padding: 0 6px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .playing {
    display: inline-block;
    width: 14px;
    margin-right: 2px;
    color: var(--accent);
    font-size: 9px;
  }
  .songs:focus-within .row.sel .playing {
    color: inherit;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .c-rating {
    color: var(--star);
    letter-spacing: 1px;
    font-size: 10px;
  }
  .songs:focus-within .row.sel .c-rating {
    color: inherit;
  }
  .empty {
    position: absolute;
    inset: 40% 0 auto;
    text-align: center;
    color: var(--muted);
    font-size: 13px;
  }
</style>
