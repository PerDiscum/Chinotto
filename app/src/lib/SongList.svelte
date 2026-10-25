<script lang="ts">
  import { t, tn } from "./i18n";
  import { store, isMac } from "./store.svelte";
  import { bitrate, duration, formatName, stars, count } from "./format";
  import type { SortKey, Track } from "./types";
  import { allowDragScroll, songMenu, startSongDrag } from "./trackActions";

  let { tracks }: { tracks: Track[] } = $props();

  // The iPod's Podcasts playlist names its columns for episodes.
  const podcastList = $derived.by(() => {
    const src = store.source;
    return src.kind === "playlist" && !!store.ipod?.playlists.find((p) => p.id === src.id)?.podcasts;
  });
  const label = (c: { key: SortKey; label: string }) =>
    podcastList && c.key === "title" ? t("Episode Title") : podcastList && c.key === "album" ? t("Podcast") : c.label;

  const ROW = 20;
  const OVERSCAN = 8;
  type Col = { key: SortKey; label: string; width: string; min: number; num?: boolean; fixed?: boolean };
  // Default order; the user can drag the titles to rearrange them ("#" and Name stay first).
  const ALL: Col[] = [
    { key: "order", label: "#", width: "38px", min: 38, num: true, fixed: true },
    { key: "title", label: t("Name"), width: "minmax(180px, 2.4fr)", min: 180, fixed: true },
    { key: "artist", label: t("Artist"), width: "minmax(120px, 1.5fr)", min: 120 },
    { key: "year", label: t("Year"), width: "46px", min: 46, num: true },
    { key: "album", label: t("Album"), width: "minmax(120px, 1.5fr)", min: 120 },
    { key: "disc", label: t("Disc"), width: "40px", min: 40, num: true },
    { key: "track", label: t("Track"), width: "44px", min: 44, num: true },
    { key: "format", label: t("Format"), width: "58px", min: 58 },
    { key: "bitrate", label: t("Bit Rate"), width: "72px", min: 72, num: true },
    { key: "time", label: t("Time"), width: "52px", min: 52, num: true },
    { key: "genre", label: t("Genre"), width: "minmax(80px, 0.9fr)", min: 80 },
    { key: "rating", label: t("Rating"), width: "76px", min: 76 },
    { key: "plays", label: t("Plays"), width: "50px", min: 50, num: true },
    { key: "path", label: t("Path"), width: "minmax(160px, 2fr)", min: 160 },
  ];
  const FIXED = ALL.filter((c) => c.fixed);
  const MOVABLE = ALL.filter((c) => !c.fixed);
  const ordered = $derived([...FIXED, ...store.columnsInOrder(MOVABLE.map((c) => c.key)).map((k) => MOVABLE.find((c) => c.key === k)!)]);
  // "#" only where the list has an order of its own (a playlist).
  const columns = $derived(ordered.filter((c) => (c.key === "order" ? store.hasOwnOrder : !store.hiddenColumns.includes(c.key))));
  const template = $derived(columns.map((c) => c.width).join(" "));
  const minWidth = $derived(columns.reduce((n, c) => n + c.min, 0));

  /** Right-click the column titles: choose which columns show. */
  function chooseColumns(e: MouseEvent) {
    e.preventDefault();
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: ALL.filter((c) => !c.fixed).map((c) => ({
        label: `${store.hiddenColumns.includes(c.key) ? "\u2003" : "✓ "}${label(c)}`,
        action: () => store.toggleColumn(c.key),
      })),
    };
  }

  /** iPod locations are written ":iPod_Control:Music:F00:X.mp3". */
  const showPath = (p?: string) => (!p ? "" : p.startsWith(":") ? p.replaceAll(":", "/") : p);

  // ---- Dragging a column title to a new place ----
  let colDrag = $state<{ key: string; before: string | null; x: number } | null>(null);
  let head: HTMLDivElement | undefined = $state();
  function pressTitle(e: MouseEvent, c: Col) {
    if (e.button !== 0 || c.fixed || !head) return;
    const x0 = e.clientX;
    const move = (m: MouseEvent) => {
      if (!colDrag && Math.abs(m.clientX - x0) < 6) return;
      // The gap nearest the pointer, between movable titles.
      const cells = [...head!.querySelectorAll<HTMLElement>("[data-col]")].filter((el) => el.dataset.movable);
      let before: string | null = null;
      let x = cells.length ? cells[cells.length - 1].getBoundingClientRect().right : 0;
      for (const el of cells) {
        const r = el.getBoundingClientRect();
        if (m.clientX < r.left + r.width / 2) {
          before = el.dataset.col!;
          x = r.left;
          break;
        }
      }
      colDrag = { key: c.key, before, x: x - head!.getBoundingClientRect().left };
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      const d = colDrag;
      colDrag = null;
      if (!d) return;
      // The click that ends the drag doesn't sort.
      const swallow = (ev: MouseEvent) => ev.stopPropagation();
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
      if (d.before !== d.key) store.moveColumn(d.key, d.before, MOVABLE.map((x) => x.key));
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }

  function cell(c: Col, t: Track, n: number): string {
    switch (c.key) {
      case "order": return String(n + 1);
      case "time": return duration(t.lengthMs);
      case "artist": return t.artist ?? "";
      case "album": return t.album ?? "";
      case "genre": return t.genre ?? "";
      case "year": return t.year ? String(t.year) : "";
      case "format": return formatName(t);
      case "bitrate": return bitrate(t);
      case "rating": return stars(t.rating);
      case "plays": return t.playCount ? count(t.playCount) : "";
      case "disc": return t.discNr ? (t.discCount > 1 ? `${t.discNr}/${t.discCount}` : String(t.discNr)) : "";
      case "track": return t.trackNr ? String(t.trackNr) : "";
      case "path": return showPath(t.location);
      default: return "";
    }
  }

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(400);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(tracks.length, Math.ceil((scrollTop + viewport) / ROW) + OVERSCAN));
  const slice = $derived(tracks.slice(first, last));

  // Back to the top when a different list is shown (playlist, search, browser pick,
  // sort), but not when the same list is saved (e.g. after reordering).
  // Only real changes count: a refreshed iPod (after copying songs to it)
  // or an equal-but-new source object keeps the place in the list.
  const ipodPath = $derived(store.ipod?.path);
  const sourceKey = $derived(JSON.stringify(store.source));
  $effect(() => {
    void [sourceKey, store.search, store.browserSel, store.sortKey, store.sortDir, ipodPath];
    if (scroller) scroller.scrollTop = 0;
  });

  // Show Playing Song: scroll the song into the middle of the list.
  $effect(() => {
    const id = store.reveal;
    if (id === null || !scroller) return;
    const i = tracks.findIndex((x) => x.id === id);
    if (i < 0) return;
    const el = scroller;
    requestAnimationFrame(() => {
      el.scrollTop = Math.max(0, i * ROW - el.clientHeight / 2);
      el.focus();
    });
    store.reveal = null;
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

  const notAvailable = t("Not available: its drive isn't connected");
  const starsLabel = (n: number) => tn(n, "{n} star", "{n} stars");
  const contextmenu = (e: MouseEvent, t: Track) => songMenu(e, t, tracks);

  /** Where in the list a drop at screen height `y` would insert (row boundary), or null outside it. */
  let edgeSince = 0;
  function insertionAt(x: number, y: number): number | null {
    if (!scroller) return null;
    const r = scroller.getBoundingClientRect();
    if (x < r.left || x > r.right || y < r.top || y > r.bottom) return null;
    // Reordering a long playlist: scroll only after resting at the very top or
    // bottom of the rows for a moment, so passing through on the way to the
    // sidebar doesn't move the list.
    const edge = y > r.top + ROW && y < r.top + ROW + 14 ? -1 : y > r.bottom - 14 ? 1 : 0;
    if (!edge) edgeSince = 0;
    else if (!edgeSince) edgeSince = performance.now();
    else if (performance.now() - edgeSince > 500) {
      allowDragScroll();
      scroller.scrollTop += edge * ROW;
    }
    const pos = (y - r.top - ROW + scroller.scrollTop) / ROW; // minus the sticky header
    return Math.max(0, Math.min(tracks.length, Math.round(pos)));
  }

  const pressForDrag = (e: MouseEvent, t: Track) => startSongDrag(e, t, tracks, store.canReorder ? insertionAt : null);
</script>

<div
  class="songs"
  role="grid"
  tabindex="0"
  aria-label={t("Songs")}
  aria-rowcount={tracks.length}
  aria-multiselectable="true"
  bind:this={scroller}
  bind:clientHeight={viewport}
  onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
  onkeydown={keydown}
>
  <div class="head row" role="row" tabindex="-1" bind:this={head} style:grid-template-columns={template} style:min-width="{minWidth}px" oncontextmenu={chooseColumns}>
    {#each columns as c (c.key)}
      <button
        class="cell"
        class:num={c.num}
        class:sorted={store.sortKey === c.key}
        class:dragging={colDrag?.key === c.key}
        data-col={c.key}
        data-movable={c.fixed ? undefined : "1"}
        onmousedown={(e) => pressTitle(e, c)}
        role="columnheader"
        aria-sort={store.sortKey === c.key ? (store.sortDir === 1 ? "ascending" : "descending") : "none"}
        onclick={() => store.sortBy(c.key)}
      >
        <span class="label">{label(c)}</span>
        {#if store.sortKey === c.key}<span class="arrow">{store.sortDir === 1 ? "▲" : "▼"}</span>{/if}
      </button>
    {/each}
    {#if colDrag}<span class="col-drop" style:left="{colDrag.x - 1}px" aria-hidden="true"></span>{/if}
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
        class:unavailable={t.unavailable}
        title={t.unavailable ? notAvailable : undefined}
        role="row"
        tabindex="-1"
        aria-rowindex={n + 1}
        aria-selected={store.selected.has(t.id)}
        style:transform="translateY({n * ROW}px)"
        style:grid-template-columns={template}
        style:min-width="{minWidth}px"
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
        {#each columns as c (c.key)}
          {#if c.key === "title"}
            <span class="cell">
              {#if store.nowPlaying === t.dbid}<span class="playing" aria-label={store.player.state === "playing" ? "Playing" : "Paused"}>{store.player.state === "playing" ? "▶" : "❙❙"}</span>{/if}{dash(t.title)}
            </span>
          {:else if c.key === "rating"}
            <span class="cell c-rating" aria-label={starsLabel(t.rating)}>{cell(c, t, n)}</span>
          {:else}
            <span class="cell" class:num={c.num} class:muted={c.key === "order"}>{cell(c, t, n)}</span>
          {/if}
        {/each}
      </div>
    {/each}
  </div>

  {#if store.songDrag?.insertAt != null}
    <div class="insert" style:top="{ROW + store.songDrag.insertAt * ROW - 1}px" style:min-width="{minWidth}px" aria-hidden="true"></div>
  {/if}

  {#if tracks.length === 0}
    <p class="empty">
      {#if store.search}{t("No songs match “{q}”.", { q: store.search })}{:else}{t("This playlist is empty.")}{/if}
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
  }
  .songs:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .row {
    display: grid;
    height: 20px;
    align-items: center;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--header);
    border-bottom: 1px solid var(--edge);
  }
  .head .cell {
    display: flex;
    align-items: center;
    gap: 4px;
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
  .head .cell .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .head .cell.num .label {
    text-align: right;
  }
  .head .cell.dragging {
    opacity: 0.5;
  }
  .col-drop {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--accent);
    pointer-events: none;
  }
  .arrow {
    flex: none;
    font-size: 8px;
  }
  .muted {
    color: var(--muted);
  }
  .body {
    position: relative;
  }
  .insert {
    position: absolute;
    left: 0;
    right: 0;
    z-index: 2;
    height: 2px;
    background: var(--accent);
    pointer-events: none;
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
  .body .row.unavailable {
    color: var(--muted);
    font-style: italic;
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
