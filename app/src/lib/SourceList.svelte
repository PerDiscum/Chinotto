<script lang="ts">
  import { t, tn } from "./i18n";
  import { store, isMac } from "./store.svelte";
  import Icon from "./Icon.svelte";
  import { deviceImage } from "./deviceImages";

  const playlists = $derived(store.ipod?.playlists.filter((p) => !p.master) ?? []);
  const isOpen = (path: string) => store.ipod?.path === path;
  const isPlaylist = (id: string) => store.source.kind === "playlist" && store.source.id === id;

  function playlistMenu(e: MouseEvent, id: string, name: string) {
    e.preventDefault();
    // Smart playlists (made by iTunes) can be renamed and deleted, not edited.
    const ro = !store.ipod?.writable;
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: t("Rename"), disabled: ro, action: () => (store.renaming = id) },
        { label: t("Delete Playlist…"), danger: true, disabled: ro, action: () => (store.dialog = { kind: "delete-playlist", id, name }) },
      ],
    };
  }

  function renameKey(e: KeyboardEvent, id: string) {
    if (e.key === "Enter") store.renamePlaylist(id, (e.currentTarget as HTMLInputElement).value);
    else if (e.key === "Escape") store.renaming = null;
  }

  function rowKey(e: KeyboardEvent, id: string, name: string) {
    if (!store.ipod?.writable) return;
    if (e.key === "Enter") {
      e.preventDefault();
      store.renaming = id;
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      store.dialog = { kind: "delete-playlist", id, name };
    }
  }

  const isLib = (lib: string, id = "all") => store.source.kind === "library" && store.source.lib === lib && store.source.id === id;

  function libraryMenu(e: MouseEvent, id: string, name: string) {
    e.preventDefault();
    const lib = store.libraries.find((l) => l.id === id);
    const missing = lib?.folders.some((f) => !f.available) ?? false;
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: lib?.scanning ? t("Scanning…") : t("Scan Now"), disabled: !!lib?.scanning, action: () => store.scanLibrary(id) },
        { label: t("Read All Songs Again"), disabled: !!lib?.scanning, action: () => store.scanLibrary(id, true) },
        { label: t("New Playlist"), disabled: !store.libraryPlaylistsWritable(id), action: () => store.newLibraryPlaylist(id) },
        { label: t("Edit Library…"), action: () => (store.dialog = { kind: "library", id }) },
        { label: t("Remove Unavailable Songs"), disabled: !missing, action: () => store.removeUnavailable(id) },
        { label: "", separator: true },
        { label: t("Delete Library…"), danger: true, action: () => (store.dialog = { kind: "delete-library", id, name }) },
      ],
    };
  }

  function deviceLibraryMenu(e: MouseEvent, lib: string) {
    e.preventDefault();
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [{ label: t("New Playlist"), disabled: !store.libraryPlaylistsWritable(lib), action: () => store.newLibraryPlaylist(lib) }],
    };
  }

  function libPlaylistMenu(e: MouseEvent, lib: string, scoped: string, name: string) {
    e.preventDefault();
    const ro = !store.libraryPlaylistsWritable(lib);
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: t("Rename"), disabled: ro, action: () => (store.renaming = scoped) },
        { label: t("Delete Playlist…"), danger: true, disabled: ro, action: () => (store.dialog = { kind: "delete-playlist", id: scoped, name }) },
      ],
    };
  }

  function libTitle(l: (typeof store.libraries)[number]): string {
    const off = l.folders.filter((f) => !f.available).length;
    const p = store.scanProgress[l.id];
    if (l.scanning && p?.total) return t("Scanning: {done} of {total}", { done: p.done, total: p.total });
    if (l.scanning) return t("Scanning…");
    const songs = tn(l.songs, "{n} song", "{n} songs");
    return off ? `${songs}; ${tn(off, "{n} folder not connected", "{n} folders not connected")}` : songs;
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<nav class="sources" aria-label={t("Sources")}>
  {#if store.scratch}
    <h2>{t("Scratch")}</h2>
    <ul class="libs">
      <li>
        <div class="row device" class:active={store.source.kind === "scratch"}>
          <button class="name" onclick={() => store.select({ kind: "scratch" })} title={t("Songs you opened, not added to any library. Forgotten when Chinotto quits.")}>
            <Icon name="music" />
            <span>{tn(store.scratch.tracks.length, "{n} song", "{n} songs")}</span>
          </button>
          <button class="eject" title={t("Clear the Scratch list")} aria-label={t("Clear the Scratch list")} onclick={() => store.removeFromScratch([])}>×</button>
        </div>
      </li>
    </ul>
  {/if}
  {#if store.ownLibraries.length}
    <h2>{t("Libraries")}</h2>
    <ul class="libs">
      {#each store.ownLibraries as l (l.id)}
        {@const open = !store.collapsed.includes(l.id)}
        {@const pls = store.libViews[l.id]?.view.playlists.filter((p) => !p.master) ?? []}
        <li>
          <div class="row device" class:active={isLib(l.id)}>
            <button
              class="disclose"
              aria-label={open ? t("Collapse {name}", { name: l.name }) : t("Expand {name}", { name: l.name })}
              aria-expanded={open}
              onclick={() => store.toggleCollapsed(l.id)}>{open ? "▾" : "▸"}</button
            >
            <button class="name" onclick={() => store.showLibrary(l.id)} oncontextmenu={(e) => libraryMenu(e, l.id, l.name)}>
              <Icon name="library" />
              <span>{l.name}</span>
              {#if l.scanning}<span class="spinner" aria-label={t("Scanning")}></span>{/if}
              {#if l.folders.some((f) => !f.available)}<span class="off" title={libTitle(l)} aria-label={t("Some folders not connected")}>◌</span>{/if}
            </button>
          </div>
          {#if open && pls.length}
            <ul class="nested">
              {#each pls as p (p.id)}
                {@const scoped = `lib:${l.id}:${p.id}`}
                <li>
                  {#if store.renaming === scoped}
                    <div class="row active">
                      <Icon name="playlist" />
                      <input
                        class="rename"
                        value={p.name}
                        aria-label={t("Playlist name")}
                        use:focusSelect
                        onkeydown={(e) => renameKey(e, scoped)}
                        onblur={(e) => store.renaming === scoped && store.renamePlaylist(scoped, e.currentTarget.value)}
                      />
                    </div>
                  {:else}
                    <button
                      class="row"
                      class:active={isLib(l.id, p.id)}
                      class:drop={store.songDrag?.target === scoped}
                      data-drop-playlist={store.libraryPlaylistsWritable(l.id) ? scoped : undefined}
                      onclick={() => store.showLibrary(l.id, p.id)}
                      ondblclick={() => store.libraryPlaylistsWritable(l.id) && (store.renaming = scoped)}
                      oncontextmenu={(e) => libPlaylistMenu(e, l.id, scoped, p.name)}
                    >
                      <Icon name="playlist" /><span>{p.name}</span>
                    </button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  <h2>{t("Optical")}</h2>
  <ul class="optical">
      {#each store.discs as d (d.id)}
        <li>
          <div class="row device" class:active={store.source.kind === "disc" && store.source.id === d.id}>
            <button class="name" onclick={() => store.select({ kind: "disc", id: d.id })} title={t("Rip this CD")}>
              <Icon name="disc" />
              <span>{store.discDrafts[d.id]?.album || d.name}</span>
            </button>
            <button class="eject" title={t("Eject {name}", { name: d.name })} aria-label={t("Eject {name}", { name: d.name })} disabled={!!store.busy} onclick={() => store.ejectDisc(d.id)}>
              <Icon name="eject" size={13} />
            </button>
          </div>
        </li>
      {/each}
      {#if !isMac && !store.discs.length}
        <!-- Linux: the drive isn't read until asked (it spins up). -->
        <li><div class="row device"><button class="name" onclick={() => store.showDisc()}><Icon name="disc" /><span>{t("Rip CD…")}</span></button></div></li>
      {/if}
      <li>
        <div class="row device" class:active={store.source.kind === "burn"}>
          <button class="name" onclick={() => store.select({ kind: "burn" })} title={t("Burn an .iso or .bin/.cue onto a blank CD or DVD")}>
            <Icon name="burn" />
            <span>{t("Burn Image")}</span>
          </button>
        </div>
      </li>
    </ul>
  <h2>{t("Devices")}</h2>
  {#if store.devices.length === 0}
    <p class="hint">{t("Connect your device, and it will show up here.")}</p>
  {/if}
  <ul>
    {#each store.devices as d (d.path)}
      <li>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="row device"
          class:active={(isOpen(d.path) && store.source.kind === "device") || ((store.source.kind === "player" || store.source.kind === "openmg") && store.source.path === d.path)}
          class:drop={store.songDrag?.target === `player:${d.path}`}
          data-drop-playlist={d.kind === "msc" || d.kind === "openmg" || (d.kind === "mtp" && d.library) ? `player:${d.path}` : undefined}
          oncontextmenu={(e) => {
            e.preventDefault();
            const lib = d.library;
            store.menu = {
              x: e.clientX,
              y: e.clientY,
              items: d.kind === "openmg"
                ? [{ label: t("Show Device Page"), action: () => store.select({ kind: "openmg", path: d.path }) }]
                : d.kind === "mtp"
                ? [{ label: t("Let Other Programs Use It"), disabled: !store.mtpInfos[d.path], action: () => store.closeMtp(d.path) }]
                : lib
                ? [
                    { label: t("Scan Now"), action: () => store.scanLibrary(lib) },
                    { label: t("Read All Songs Again"), action: () => store.scanLibrary(lib, true) },
                    { label: t("Player Settings…"), action: () => (store.dialog = { kind: "msc", path: d.path }) },
                  ]
                : d.needsSetup
                ? [{ label: t("Set Up iPod…"), action: () => (store.dialog = { kind: "setup", path: d.path, rebuild: false }) }]
                : [{ label: t("Rebuild Database…"), danger: true, action: () => (store.dialog = { kind: "setup", path: d.path, rebuild: true }) }],
            };
          }}
        >
          <button
            class="name"
            onclick={() =>
              d.kind === "openmg" ? store.select({ kind: "openmg", path: d.path }) : d.kind === "mtp" ? store.openMtp(d.path) : d.library ? store.select({ kind: "player", path: d.path }) : isOpen(d.path) ? store.select({ kind: "device" }) : store.open(d.path)}
            title={d.path}
          >
            {#if deviceImage(store.iconFor(d))}<img class="dev" src={deviceImage(store.iconFor(d))} alt="" />{:else}<Icon name="ipod" />{/if}
            <span>{isOpen(d.path) ? store.ipod!.name : d.volume}</span>
            {#if d.needsSetup}<span class="setup">{t("Set Up…")}</span>{/if}
            {#if store.opening === d.path}<span class="spinner" aria-label={t("Reading")}></span>{/if}
          </button>
          {#if !d.manual}
            <button class="eject" title={t("Eject {name}", { name: d.volume })} aria-label={t("Eject {name}", { name: d.volume })} disabled={!!store.ejecting} onclick={() => store.eject(d.path)}>
              <Icon name="eject" size={13} />
            </button>
          {/if}
        </div>

        {#if d.library && store.libViews[d.library]}
          {@const lib = d.library}
          <ul class="nested">
            <li>
              <button class="row" class:active={isLib(lib)} class:drop={store.songDrag?.target === `player:${d.path}`} onclick={() => store.showLibrary(lib)} oncontextmenu={(e) => deviceLibraryMenu(e, lib)}>
                <Icon name="music" /><span>{t("Music")}</span>
              </button>
            </li>
            {#each store.libViews[lib].view.playlists.filter((p) => !p.master) as p (p.id)}
              {@const scoped = `lib:${lib}:${p.id}`}
              <li>
                {#if store.renaming === scoped}
                  <div class="row active">
                    <Icon name="playlist" />
                    <input class="rename" value={p.name} aria-label={t("Playlist name")} use:focusSelect onkeydown={(e) => renameKey(e, scoped)} onblur={(e) => store.renaming === scoped && store.renamePlaylist(scoped, e.currentTarget.value)} />
                  </div>
                {:else}
                  <button
                    class="row"
                    class:active={isLib(lib, p.id)}
                    class:drop={store.songDrag?.target === scoped}
                    data-drop-playlist={store.libraryPlaylistsWritable(lib) ? scoped : undefined}
                    onclick={() => store.showLibrary(lib, p.id)}
                    ondblclick={() => store.libraryPlaylistsWritable(lib) && (store.renaming = scoped)}
                    oncontextmenu={(e) => libPlaylistMenu(e, lib, scoped, p.name)}
                  >
                    <Icon name="playlist" /><span>{p.name}</span>
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if isOpen(d.path) && store.masterPlaylist}
          <ul class="nested">
            <li>
              <button class="row" class:drop={store.songDrag?.target === "ipod:music"} data-drop-playlist={store.ipod?.writable ? "ipod:music" : undefined} class:active={isPlaylist(store.masterPlaylist.id)} onclick={() => store.select({ kind: "playlist", id: store.masterPlaylist!.id })}>
                <Icon name="music" /><span>{t("Music")}</span>
              </button>
            </li>
            {#each playlists as p (p.id)}
              <li>
                {#if store.renaming === p.id}
                  <div class="row active">
                    <Icon name={p.smart ? "smart" : "playlist"} />
                    <input
                      class="rename"
                      value={p.name}
                      aria-label={t("Playlist name")}
                      use:focusSelect
                      onkeydown={(e) => renameKey(e, p.id)}
                      onblur={(e) => store.renaming === p.id && store.renamePlaylist(p.id, e.currentTarget.value)}
                    />
                  </div>
                {:else}
                  <button
                    class="row"
                    class:active={isPlaylist(p.id)}
                    class:drop={store.songDrag?.target === p.id}
                    data-drop-playlist={!!store.ipod?.writable && !p.smart ? p.id : undefined}
                    onclick={() => store.select({ kind: "playlist", id: p.id })}
                    ondblclick={() => !!store.ipod?.writable && (store.renaming = p.id)}
                    oncontextmenu={(e) => playlistMenu(e, p.id, p.name)}
                    onkeydown={(e) => rowKey(e, p.id, p.name)}
                  >
                    <Icon name={p.smart ? "smart" : "playlist"} /><span>{p.name}</span>
                  </button>
                {/if}
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
  .row.drop {
    background: var(--accent-bright);
    color: #2b1608;
  }
  .rename {
    flex: 1;
    min-width: 0;
    height: 20px;
    padding: 0 4px;
    border: 1px solid var(--accent-bright);
    border-radius: 4px;
    background: var(--side-deep);
    color: var(--side-ink);
    font: inherit;
    font-size: 13px;
    user-select: text;
    -webkit-user-select: text;
  }
  .libs .device .name {
    padding-left: 2px;
  }
  .disclose {
    flex: none;
    width: 14px;
    height: 100%;
    margin-left: 4px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--side-muted);
    font-size: 10px;
  }
  .off {
    flex: none;
    margin-left: auto;
    color: var(--side-muted);
    font-size: 11px;
  }
  .libs + h2 {
    margin-top: 14px;
  }
  .dev {
    flex: none;
    width: 18px;
    height: 18px;
    object-fit: contain;
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
  .setup {
    margin-left: auto;
    flex: none;
    font-size: 11px;
    color: var(--accent);
  }
</style>
