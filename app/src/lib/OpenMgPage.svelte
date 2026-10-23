<script lang="ts">
  import { t } from "./i18n";
  import { store } from "./store.svelte";
  import { bytes } from "./format";
  import { deviceImage } from "./deviceImages";

  let { path }: { path: string } = $props();
  const device = $derived(store.devices.find((d) => d.path === path));
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
  @container (max-width: 600px) { .picture { display: none; } }
</style>
