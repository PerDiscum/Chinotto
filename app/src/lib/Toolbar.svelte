<script lang="ts">
  import { store } from "./store.svelte";
  import { count, longDuration } from "./format";

  let { visibleCount, visibleMs }: { visibleCount: number; visibleMs: number } = $props();

  const openingName = $derived(
    store.opening ? (store.devices.find((d) => d.path === store.opening)?.volume ?? "iPod") : null,
  );
  const sourceName = $derived.by(() => {
    const s = store.source;
    if (s.kind === "device") return store.ipod?.name ?? "";
    if (s.kind === "playlist") {
      const p = store.ipod?.playlists.find((x) => x.id === s.id);
      return p?.master ? `${store.ipod?.name} — Music` : (p?.name ?? "");
    }
    return "";
  });
</script>

<header class="toolbar" data-tauri-drag-region>
  <div class="spacer" data-tauri-drag-region></div>

  <div class="label" role="status" aria-live="polite">
    {#if openingName}
      <strong>Reading {openingName}…</strong>
      <span class="sub">Loading songs and playlists</span>
    {:else if store.ejecting}
      <strong>Ejecting…</strong>
      <span class="sub">Don't unplug yet</span>
    {:else if store.ipod}
      <strong>{sourceName}</strong>
      <span class="sub">
        {#if store.source.kind === "device"}
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
      disabled={!store.ipod}
      spellcheck="false"
      autocomplete="off"
    />
  </div>
</header>

<style>
  .toolbar {
    grid-area: toolbar;
    display: grid;
    grid-template-columns: 1fr minmax(320px, 460px) 1fr;
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
    min-height: 42px;
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
