<script lang="ts">
  import { store, type MusicView } from "./store.svelte";

  const TABS: [MusicView, string][] = [
    ["songs", "Songs"],
    ["albums", "Albums"],
    ["artists", "Artists"],
    ["genres", "Genres"],
    ["composers", "Composers"],
    ["compilations", "Compilations"],
  ];
</script>

<div class="tabs" role="tablist" aria-label="View">
  {#each TABS as [v, label] (v)}
    <button role="tab" aria-selected={store.view === v} class:on={store.view === v} onclick={() => store.setView(v)}>{label}</button>
  {/each}
</div>

<style>
  .tabs {
    flex: none;
    display: flex;
    justify-content: center;
    gap: 4px;
    padding: 6px 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--edge);
  }
  button {
    padding: 3px 12px;
    border: 0;
    border-radius: 999px;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
  }
  button:hover {
    background: var(--zebra);
  }
  button.on {
    background: var(--sel-quiet);
    box-shadow: inset 0 0 0 1px var(--edge);
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
