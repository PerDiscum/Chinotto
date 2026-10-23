<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import { bytes, longDuration } from "./format";
  import { deviceImage, PICTURES } from "./deviceImages";

  // The page of a Sony OpenMG Walkman. Its songs are its "Music" list in the sidebar.
  let { path }: { path: string } = $props();
  const device = $derived(store.devices.find((d) => d.path === path));
  const st = $derived(store.playerStates[path]);
  const view = $derived(device?.library ? store.libViews[device.library]?.view : undefined);
  const songs = $derived(view?.tracks.length ?? 0);
  const ms = $derived((view?.tracks ?? []).reduce((a, x) => a + x.lengthMs, 0));
  const identity = $derived(device?.identity ?? null);
  const chosen = $derived(identity ? store.deviceIcons[identity] : undefined);
  let choosing = $state(false);
  function pick(key: string | null) {
    if (identity) void store.setDeviceIcon(identity, key);
    choosing = false;
  }
  let pictureEl: HTMLElement | undefined = $state();
  function outside(e: MouseEvent) {
    if (choosing && pictureEl && !pictureEl.contains(e.target as Node)) choosing = false;
  }
</script>

<svelte:window onclick={outside} />

{#if device}
  <div class="page">
    <div class="top">
      <section class="summary">
        <h1>{device.volume}</h1>
        <dl>
          <dt>{t("Model")}</dt><dd>{device.model ?? t("Sony Walkman (OpenMG)")}</dd>
          {#if device.capacity}<dt>{t("Capacity")}</dt><dd>{bytes(device.capacity)}</dd>{/if}
          <dt>{t("Contents")}</dt><dd>{tn(songs, "{n} song", "{n} songs")}, {longDuration(ms)}</dd>
          <dt>{t("Database")}</dt><dd>OMGAUDIO / OpenMG</dd>
          <dt>{t("Location")}</dt><dd class="path">{device.path}</dd>
        </dl>
        <div class="actions">
          {#if !device.manual}<button onclick={() => store.eject(device.path)} disabled={!!store.ejecting || !!store.busy}>{t("Eject")}</button>{/if}
          {#if st?.writable}
            <button onclick={() => store.pickAndCopyToPlayer(path)} disabled={!!store.busy}>{t("Add Songs…")}</button>
            <button onclick={() => store.reorderOpenMg(path)} disabled={!!store.busy} title={t("Albums play in track order; songs added by other programs get their numbers from your libraries.")}>{t("Put Albums in Track Order")}</button>
          {/if}
        </div>
        {#if st}
          <div class="changes">
            <label class="switch">
              <input type="checkbox" role="switch" checked={st.writable} onchange={(e) => store.setPlayerAllow(path, e.currentTarget.checked)} disabled={!!store.busy} />
              <span class="track" aria-hidden="true"><span class="knob"></span></span>
              {t("Allow changes to this device.")}
            </label>
            <p class="note">
              {#if st.writable}
                {t("Chinotto can add songs to this Walkman (drag them onto it in the sidebar, or use Add Songs…), and change and delete them in its Music list. Songs go as MP3: other formats are converted to 320 kbps. Its database is backed up on the Walkman, in “Chinotto Backups”, before every change.")}
              {:else}
                {t("Chinotto only reads this Walkman: its songs are in its Music list in the sidebar, and play from there. Turn this on to add, change and delete songs. The setting is remembered for this Walkman only.")}
              {/if}
            </p>
          </div>
        {/if}
      </section>
      <div class="picture" bind:this={pictureEl}>
        <button class="device" onclick={() => (choosing = !choosing)} disabled={!identity} aria-expanded={choosing} title={t("Choose a picture for this Walkman")}>
          <img src={deviceImage(store.iconFor(device))} alt={device.model ?? "Sony Walkman"} />
        </button>
        {#if choosing}
          <div class="chooser" role="dialog" aria-label={t("Choose a picture")}>
            <button class="auto" class:on={!chosen} onclick={() => pick(null)}>{t("Use automatic")}</button>
            <div class="grid">
              {#each PICTURES as p (p.key)}
                <button class:on={chosen === p.key} onclick={() => pick(p.key)} title={p.label} aria-label={p.label}>
                  <img src={p.url} alt="" />
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .page { flex: 1; overflow: auto; padding: 32px 40px; background: var(--bg); }
  .top { display: flex; align-items: flex-start; justify-content: space-between; gap: 24px; max-width: 820px; container-type: inline-size; }
  .summary { max-width: 640px; min-width: 0; }
  .picture { position: relative; flex: none; max-width: 40%; }
  .device { display: block; padding: 0; border: 0; border-radius: 8px; background: none; cursor: pointer; }
  .device:disabled { cursor: default; }
  .device:hover:not(:disabled) img, .device:focus-visible img { filter: drop-shadow(0 0 6px rgb(0 0 0 / 0.25)); }
  .device:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .device img { display: block; width: auto; height: 180px; max-width: 100%; object-fit: contain; }
  .chooser { position: absolute; top: calc(100% + 8px); right: 0; z-index: 30; width: 316px; padding: 8px; border: 1px solid var(--edge); border-radius: 10px; background: var(--bg); box-shadow: 0 10px 30px rgb(0 0 0 / 0.25); }
  .chooser .auto { width: 100%; margin-bottom: 8px; padding: 5px 8px; border: 1px solid var(--edge); border-radius: 6px; background: none; color: var(--ink); font: inherit; font-size: 12px; text-align: left; }
  .chooser .grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 4px; }
  .chooser .grid button { display: grid; place-items: center; height: 64px; padding: 4px; border: 1px solid transparent; border-radius: 6px; background: none; }
  .chooser .grid img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .chooser button:hover { background: var(--zebra); }
  .chooser button.on { border-color: var(--accent); background: var(--sel-quiet); }
  h1 { margin: 0 0 18px; font-size: 26px; }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 8px 20px; margin: 0; }
  dt { color: var(--muted); text-align: right; }
  dd { margin: 0; min-width: 0; }
  .path { overflow-wrap: anywhere; }
  .note { max-width: 640px; margin: 18px 0 0; color: var(--muted); line-height: 1.55; }
  .actions { display: flex; gap: 8px; margin-top: 24px; }
  @container (max-width: 600px) { .picture { display: none; } }
  .changes { margin-top: 22px; padding-top: 16px; border-top: 1px solid var(--edge); }
  .switch { display: inline-flex; align-items: center; gap: 10px; font-size: 13px; font-weight: 500; }
  .switch input { position: absolute; opacity: 0; width: 1px; height: 1px; }
  .switch .track { position: relative; width: 34px; height: 20px; border-radius: 10px; background: var(--cap-other); transition: background 0.15s; }
  .switch .knob { position: absolute; top: 2px; left: 2px; width: 16px; height: 16px; border-radius: 50%; background: #fff; box-shadow: 0 1px 2px rgb(0 0 0 / 0.3); transition: transform 0.15s; }
  .switch input:checked + .track { background: var(--accent); }
  .switch input:checked + .track .knob { transform: translateX(14px); }
  .switch input:focus-visible + .track { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) { .switch .track, .switch .knob { transition: none; } }
</style>
