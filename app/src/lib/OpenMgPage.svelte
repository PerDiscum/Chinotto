<script lang="ts">
  import { t } from "./i18n";
  import { store } from "./store.svelte";
  import { bytes, duration } from "./format";
  import { deviceImage } from "./deviceImages";
  import { openMgView } from "./api";
  import type { OpenMgTrack } from "./types";

  let { path }: { path: string } = $props();
  const device = $derived(store.devices.find((d) => d.path === path));
  let tracks = $state<OpenMgTrack[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  const st = $derived(store.playerStates[path]);
  $effect(() => {
    void store.openMgRevision;
    const currentPath = path;
    loading = true;
    error = null;
    void openMgView(currentPath).then((view) => {
      if (path === currentPath) tracks = view.tracks;
    }).catch((e: unknown) => {
      if (path === currentPath) error = String(e);
    }).finally(() => {
      if (path === currentPath) loading = false;
    });
  });
</script>

{#if device}
  <div class="page">
    <div class="top">
      <section class="summary">
        <h1>{device.volume}</h1>
        <dl>
          <dt>{t("Model")}</dt><dd>{device.model ?? t("Sony Walkman (OpenMG)")}</dd>
          {#if device.capacity}<dt>{t("Capacity")}</dt><dd>{bytes(device.capacity)}</dd>{/if}
          <dt>{t("Database")}</dt><dd>OMGAUDIO / OpenMG</dd>
          <dt>{t("Location")}</dt><dd class="path">{device.path}</dd>
        </dl>
        <p class="note">{t("Chinotto recognizes this Walkman as a Sony OpenMG device. Its music and database are stored in OMGAUDIO, not in the regular MUSIC folder.")}</p>
        <div class="actions">
          {#if !device.manual}<button onclick={() => store.eject(device.path)} disabled={!!store.ejecting || !!store.busy}>{t("Eject")}</button>{/if}
          {#if st?.writable}<button onclick={() => store.pickAndCopyToPlayer(path)} disabled={!!store.busy}>{t("Add Songs…")}</button>{/if}
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
                {t("Chinotto can add songs to this Walkman (drag them onto it in the sidebar, or use Add Songs…). They go as MP3: other formats are converted to 320 kbps. Its database is backed up on the Walkman, in “Chinotto Backups”, before every change.")}
              {:else}
                {t("Chinotto only reads this Walkman. Turn this on to add songs to it. The setting is remembered for this Walkman only.")}
              {/if}
            </p>
          </div>
        {/if}
      </section>
      <div class="picture"><img src={deviceImage(device.icon)} alt={device.model ?? "Sony Walkman"} /></div>
    </div>
    <section class="tracks">
      <h2>{t("Songs on this Walkman")}</h2>
      {#if loading}
        <p class="note">{t("Reading the Walkman database…")}</p>
      {:else if error}
        <p class="note error" role="alert">{t("Could not read the Walkman track list: {error}", { error })}</p>
      {:else if tracks.length === 0}
        <p class="note">{t("No songs are listed in the Walkman database.")}</p>
      {:else}
        <div class="table-wrap">
          <table>
            <thead><tr><th>{t("Title")}</th><th>{t("Artist")}</th><th>{t("Album")}</th><th>{t("Genre")}</th><th>{t("Time")}</th><th>{t("Audio file")}</th></tr></thead>
            <tbody>
              {#each tracks as track (track.number)}
                <tr><td>{track.title ?? t("Untitled")}</td><td>{track.artist ?? "—"}</td><td>{track.album ?? "—"}</td><td>{track.genre ?? "—"}</td><td>{duration(track.lengthMs)}</td><td class:missing={!track.audioFile}>{track.audioFile ?? t("No matching OMA file")}</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
      <p class="note">{t("Audio file names are matched through the OpenMG track keys. Deleting and editing songs on this Walkman come next.")}</p>
    </section>
  </div>
{/if}

<style>
  .page { flex: 1; overflow: auto; padding: 32px 40px; background: var(--bg); }
  .top { display: flex; align-items: flex-start; justify-content: space-between; gap: 24px; max-width: 820px; container-type: inline-size; }
  .summary { max-width: 640px; min-width: 0; }
  .picture { flex: none; max-width: 40%; }
  .picture img { display: block; width: auto; height: 180px; max-width: 100%; object-fit: contain; }
  h1 { margin: 0 0 18px; font-size: 26px; }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 8px 20px; margin: 0; }
  dt { color: var(--muted); text-align: right; }
  dd { margin: 0; min-width: 0; }
  .path { overflow-wrap: anywhere; }
  .note { max-width: 640px; margin: 18px 0 0; color: var(--muted); line-height: 1.55; }
  .actions { display: flex; gap: 8px; margin-top: 24px; }
  .tracks { max-width: 1100px; margin-top: 32px; }
  h2 { margin: 0 0 12px; font-size: 18px; }
  .table-wrap { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; text-align: left; }
  th, td { padding: 9px 12px; border-bottom: 1px solid var(--chrome-edge); }
  th { color: var(--muted); font-weight: 600; }
  td:last-child, th:last-child { text-align: right; white-space: nowrap; }
  td.missing { color: var(--danger, #e07862); }
  .error { color: var(--danger, #e07862); }
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
