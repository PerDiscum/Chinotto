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
  $effect(() => {
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
        <p class="note">{t("Generic drive setup and writing are disabled to protect the device database. OpenMG initialization and song transfer are not available in this version yet.")}</p>
        <div class="actions">
          {#if !device.manual}<button onclick={() => store.eject(device.path)} disabled={!!store.ejecting}>{t("Eject")}</button>{/if}
        </div>
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
      <p class="note">{t("Audio file names are matched through the OpenMG track keys. This remains a read-only view; Chinotto does not change OpenMG devices yet.")}</p>
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
</style>
