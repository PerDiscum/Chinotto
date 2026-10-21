<script lang="ts">
  import { t } from "./i18n";
  import { untrack } from "svelte";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import * as api from "./api";
  import { PICTURES, deviceImage } from "./deviceImages";
  import type { DriveInfo, MscConfig } from "./types";

  // Add a player that shows up as a drive (MSC: most Walkmans and Sansas), or
  // change its settings. They're kept on the player, so it's recognised
  // whenever it's plugged in.
  let { path }: { path: string } = $props();

  let info = $state<DriveInfo | null>(null);
  let failed = $state<string | null>(null);
  let cfg = $state<MscConfig>({ id: "", name: "", model: "other", icon: "generic", musicFolder: "Music", playlistFolder: "Playlists", playlistFormat: "m3u8" });
  let confirmed = $state(false);

  $effect(() => {
    untrack(() => {
      api
        .mscInspect(path)
        .then((i) => {
          info = i;
          if (i.config) cfg = { ...i.config };
          else {
            cfg.name = i.volume;
            applyProfile(i.suggested, i);
          }
        })
        .catch((e) => (failed = String(e)));
    });
  });

  /** A model's usual folders, picture and playlist format (folder names as the drive spells them). */
  function applyProfile(key: string, i = info) {
    const p = i?.profiles.find((x) => x.key === key);
    if (!p || !i) return;
    const onDrive = (f: string) => i.folders.find((x) => x.toLowerCase() === f.toLowerCase()) ?? f;
    cfg.model = p.key;
    cfg.icon = p.icon;
    cfg.musicFolder = onDrive(p.musicFolder);
    cfg.playlistFolder = onDrive(p.playlistFolder);
    cfg.playlistFormat = p.playlistFormat;
  }

  const blocked = $derived(!info || !!info.isIpod || !cfg.musicFolder.trim() || (!info.isDisk && !confirmed));
  const close = () => (store.dialog = null);
  const save = () => !blocked && void store.saveMsc(path, $state.snapshot(cfg));
</script>

<Modal title={info?.config ? t("Player Settings") : t("Add Player")} onclose={close} width={560}>
  {#if failed}
    <p class="warn">{failed}</p>
  {:else if !info}
    <p class="muted">{t("Looking at the drive…")}</p>
  {:else if info.isIpod}
    <p>{t("This is an iPod: use Open iPod Folder… instead.")}</p>
  {:else}
    <div class="drive">
      {#if deviceImage(cfg.icon)}<img src={deviceImage(cfg.icon)} alt="" />{/if}
      <div>
        <strong>{info.volume}</strong>
        <span class="muted">{info.path}</span>
      </div>
    </div>
    <p class="muted">
      {t("For players that show up as a drive (most Sony Walkmans and SanDisk Sansas). Chinotto lists the songs in the music folder and reads the playlists in the playlist folder. These settings are saved on the player, in a small hidden file (.chinotto-device.json), so it's recognised whenever it's plugged in.")}
    </p>
    {#if !info.isDisk}
      <p class="warn">{t("This is a folder, not the top of a drive. Only go on if it really is a player's drive (for example, a copy you're testing with).")}</p>
    {/if}
    <div class="form">
      <label>
        <span>{t("Model")}</span>
        <select value={cfg.model} onchange={(e) => applyProfile(e.currentTarget.value)}>
          {#each info.profiles as p (p.key)}<option value={p.key}>{p.name}</option>{/each}
        </select>
      </label>
      <label>
        <span>{t("Name")}</span>
        <input bind:value={cfg.name} />
      </label>
      <div class="two">
        <label>
          <span>{t("Music folder")}</span>
          <input bind:value={cfg.musicFolder} list="msc-folders" spellcheck="false" />
        </label>
        <label>
          <span>{t("Playlist folder")}</span>
          <input bind:value={cfg.playlistFolder} list="msc-folders" spellcheck="false" />
        </label>
      </div>
      <datalist id="msc-folders">
        {#each info.folders as f (f)}<option value={f}></option>{/each}
      </datalist>
      <div class="two">
        <label>
          <span>{t("Playlist format")}</span>
          <select bind:value={cfg.playlistFormat}>
            <option value="m3u8">M3U8 (UTF-8)</option>
            <option value="m3u">M3U</option>
            <option value="pls">PLS</option>
          </select>
        </label>
        <label>
          <span>{t("Picture")}</span>
          <select bind:value={cfg.icon}>
            {#each PICTURES as p (p.key)}<option value={p.key}>{p.label}</option>{/each}
          </select>
        </label>
      </div>
      {#if !info.isDisk}
        <label class="check">
          <input type="checkbox" bind:checked={confirmed} />
          <span>{t("Yes, this is a player's drive")}</span>
        </label>
      {/if}
    </div>
  {/if}
  {#snippet buttons()}
    <button onclick={close}>{t("Cancel")}</button>
    <button class="primary" disabled={blocked} onclick={save}>{info?.config ? t("OK") : t("Add Player")}</button>
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 12px;
    line-height: 1.5;
  }
  .muted {
    color: var(--muted);
  }
  .warn {
    color: var(--danger, #b3261e);
  }
  .drive {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .drive img {
    width: 44px;
    height: 44px;
    object-fit: contain;
  }
  .drive div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .drive span {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    min-width: 0;
  }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-weight: 400;
    font-size: 13px;
  }
  input:not([type="checkbox"]),
  select {
    font: inherit;
    font-weight: 400;
    font-size: 13px;
    padding: 4px 6px;
  }
</style>
