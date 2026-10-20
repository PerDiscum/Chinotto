<script lang="ts">
  import { store } from "./store.svelte";
  import type { BrowserColumns } from "./library";
  import { count } from "./format";

  let { columns }: { columns: BrowserColumns } = $props();

  const cols = $derived([
    { title: "Genre", plural: ["Genre", "Genres"], items: columns.genres, value: store.genre, pick: (v: string | null) => store.pickGenre(v) },
    { title: "Artist", plural: ["Artist", "Artists"], items: columns.artists, value: store.artist, pick: (v: string | null) => store.pickArtist(v) },
    { title: "Album", plural: ["Album", "Albums"], items: columns.albums, value: store.album, pick: (v: string | null) => store.pickAlbum(v) },
  ]);

  function keydown(e: KeyboardEvent, items: string[], value: string | null, pick: (v: string | null) => void) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = value === null ? -1 : items.indexOf(value);
    const n = e.key === "ArrowDown" ? Math.min(items.length - 1, i + 1) : Math.max(-1, i - 1);
    pick(n < 0 ? null : items[n]);
    const list = e.currentTarget as HTMLElement;
    queueMicrotask(() => list.querySelector<HTMLElement>(".sel")?.scrollIntoView({ block: "nearest" }));
  }
</script>

<div class="browser">
  {#each cols as c (c.title)}
    <section>
      <h3>{c.title}</h3>
      <div class="list" role="listbox" tabindex="0" aria-label={c.title} onkeydown={(e) => keydown(e, c.items, c.value, c.pick)}>
        <button role="option" aria-selected={c.value === null} class:sel={c.value === null} onclick={() => c.pick(null)}>
          All ({count(c.items.length)} {c.items.length === 1 ? c.plural[0] : c.plural[1]})
        </button>
        {#each c.items as item (item)}
          <button role="option" aria-selected={c.value === item} class:sel={c.value === item} onclick={() => c.pick(item)}>{item}</button>
        {/each}
      </div>
    </section>
  {/each}
</div>

<style>
  .browser {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    height: 168px;
    flex: none;
    border-bottom: 1px solid var(--edge);
  }
  section {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-right: 1px solid var(--edge);
  }
  section:last-child {
    border-right: 0;
  }
  h3 {
    margin: 0;
    padding: 3px 8px;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    background: var(--header);
    border-bottom: 1px solid var(--edge);
    text-align: center;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    background: var(--bg);
  }
  .list:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .list button {
    display: block;
    width: 100%;
    height: 19px;
    padding: 0 8px;
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: 12px;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }
  .list button:nth-child(even) {
    background: var(--zebra);
  }
  .list button.sel {
    background: var(--sel-quiet);
  }
  .list:focus-within button.sel {
    background: var(--sel);
    color: var(--sel-ink);
  }
</style>
