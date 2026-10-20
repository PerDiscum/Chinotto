<script lang="ts">
  import { store } from "./store.svelte";
  import Icon from "./Icon.svelte";

  const playlists = $derived(store.ipod?.playlists.filter((p) => !p.master) ?? []);
  const isOpen = (path: string) => store.ipod?.path === path;
  const isPlaylist = (id: string) => store.source.kind === "playlist" && store.source.id === id;
</script>

<nav class="sources" aria-label="Sources">
  <h2>Devices</h2>
  {#if store.devices.length === 0}
    <p class="hint">Connect an iPod and it shows up here.</p>
  {/if}
  <ul>
    {#each store.devices as d (d.path)}
      <li>
        <div class="row device" class:active={isOpen(d.path) && store.source.kind === "device"}>
          <button
            class="name"
            onclick={() => (isOpen(d.path) ? store.select({ kind: "device" }) : store.open(d.path))}
            title={d.path}
          >
            <Icon name="ipod" />
            <span>{isOpen(d.path) ? store.ipod!.name : d.volume}</span>
            {#if store.opening === d.path}<span class="spinner" aria-label="Reading"></span>{/if}
          </button>
          {#if !d.manual}
            <button class="eject" title="Eject {d.volume}" aria-label="Eject {d.volume}" disabled={!!store.ejecting} onclick={() => store.eject(d.path)}>
              <Icon name="eject" size={13} />
            </button>
          {/if}
        </div>

        {#if isOpen(d.path) && store.masterPlaylist}
          <ul class="nested">
            <li>
              <button class="row" class:active={isPlaylist(store.masterPlaylist.id)} onclick={() => store.select({ kind: "playlist", id: store.masterPlaylist!.id })}>
                <Icon name="music" /><span>Music</span>
              </button>
            </li>
            {#each playlists as p (p.id)}
              <li>
                <button class="row" class:active={isPlaylist(p.id)} onclick={() => store.select({ kind: "playlist", id: p.id })}>
                  <Icon name={p.smart ? "smart" : "playlist"} /><span>{p.name}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sources {
    flex: 1;
    overflow-y: auto;
    padding: 10px 0 12px;
  }
  h2 {
    margin: 4px 14px 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--side-muted);
  }
  .hint {
    margin: 0 14px;
    font-size: 12px;
    color: var(--side-muted);
    line-height: 1.4;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .nested .row {
    padding-left: 30px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    height: 24px;
    padding: 0 10px 0 14px;
    border: 0;
    background: none;
    color: var(--side-ink);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: default;
  }
  .row span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row :global(svg) {
    flex: none;
    color: var(--side-icon);
  }
  .device {
    padding: 0 6px 0 0;
  }
  .device .name {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
    height: 100%;
    padding-left: 14px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
  }
  .row.active,
  button.row.active {
    background: var(--side-sel);
    color: var(--side-sel-ink);
  }
  .row.active :global(svg) {
    color: inherit;
  }
  button:focus-visible {
    outline: 2px solid var(--accent-bright);
    outline-offset: -2px;
  }
  .eject {
    flex: none;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border: 0;
    border-radius: 50%;
    background: none;
    color: inherit;
    opacity: 0.75;
  }
  .eject:hover:not(:disabled) {
    opacity: 1;
    background: rgb(255 255 255 / 0.12);
  }
  .spinner {
    flex: none;
    width: 10px;
    height: 10px;
    margin-left: auto;
    border: 1.5px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 3s;
    }
  }
</style>
