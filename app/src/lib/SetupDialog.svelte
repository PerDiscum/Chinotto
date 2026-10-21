<script lang="ts">
  import { t, tn } from "./i18n";
  import { untrack } from "svelte";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import * as api from "./api";
  import { bytes } from "./format";
  import { deviceImage } from "./deviceImages";
  import type { SetupInfo } from "./types";

  // Set up an iPod that has no database (formatted or restored without
  // iTunes), or rebuild one whose database is lost or broken.
  let { path, rebuild }: { path: string; rebuild: boolean } = $props();

  let info = $state<SetupInfo | null>(null);
  let failed = $state<string | null>(null);
  let model = $state("");
  let name = $state("");
  let firewireId = $state("");
  let addExisting = $state(true);
  let confirmed = $state(false);

  $effect(() => {
    untrack(() => {
      api
        .setupInfo(path)
        .then((i) => {
          info = i;
          model = i.model && i.models.some((m) => m.number === i.model) ? i.model : "";
          name = i.volume;
          firewireId = i.firewireId ?? "";
        })
        .catch((e) => (failed = String(e)));
    });
  });

  const chosen = $derived(info?.models.find((m) => m.number === model) ?? null);
  const idOk = $derived(/^(0x)?[0-9a-fA-F]{16}$/.test(firewireId.trim()));
  const needsId = $derived(!!chosen?.needsId);
  const blocked = $derived(!info || !chosen || (needsId && !idOk) || (rebuild ? !confirmed : !!info?.hasDatabase) || (!info?.isDisk && !confirmed));

  const close = () => (store.dialog = null);
  function go() {
    if (blocked || !info) return;
    void store.setUpIpod({ path, model, name, firewireId: idOk ? firewireId.trim() : null, addExisting: addExisting && info.musicFiles > 0, rebuild });
  }
</script>

<Modal title={rebuild ? t("Rebuild the iPod's Database") : t("Set Up iPod")} onclose={close} width={560}>
  {#if failed}
    <p class="warn">{failed}</p>
  {:else if !info}
    <p class="muted">{t("Looking at the drive…")}</p>
  {:else}
    <div class="drive">
      {#if chosen && deviceImage(chosen.icon)}<img src={deviceImage(chosen.icon)} alt="" />{/if}
      <div>
        <strong>{info.volume}</strong>
        <span class="muted">{info.capacity ? bytes(info.capacity) : ""} · {info.path}</span>
      </div>
    </div>

    {#if rebuild}
      <p class="warn">
        {t("This replaces the iPod's database. The songs stay on the iPod and are added back from their files. If the old database can still be read, its playlists and ratings are kept; if it's damaged, they're lost. Play counts are always lost. The old database isn't deleted: it's moved to iPod_Control/iTunes/Chinotto Backups.")}
      </p>
    {:else if info.hasDatabase}
      <p class="warn">{t("This iPod already has a database. To replace it, use Rebuild Database (right-click the iPod in the sidebar).")}</p>
    {:else}
      <p class="muted">{t("Chinotto creates the iPod's database and folders, so it can take music. Nothing else on the drive is changed.")}</p>
    {/if}
    {#if !info.isDisk}
      <p class="warn">{t("This is a folder, not the top of a drive. Only go on if it really is an iPod's drive (for example, a copy you're testing with).")}</p>
    {/if}

    <div class="form">
      <label>
        <span>{t("Model")}</span>
        <select bind:value={model}>
          <option value="" disabled>{t("Choose the model…")}</option>
          {#each info.models as m (m.number)}<option value={m.number}>{m.name}</option>{/each}
        </select>
        {#if info.model && model === info.model}<small class="muted">{t("As the iPod reports itself.")}</small>{/if}
      </label>
      <label>
        <span>{t("Name")}</span>
        <input bind:value={name} placeholder="iPod" />
      </label>
      {#if needsId}
        <label>
          <span>{t("FireWire ID")}</span>
          <input bind:value={firewireId} placeholder="000A27001B2C3D4E" spellcheck="false" />
          {#if info.firewireId && firewireId.trim() === info.firewireId}
            <small class="muted">{t("Read from the iPod.")}</small>
          {:else if !idOk}
            <small class="warn">{t("This model needs its FireWire ID (16 hexadecimal digits), or it will show no music. Chinotto couldn't read it from the iPod; iTunes, System Information (USB) or “lsusb -v” show it as the serial number.")}</small>
          {/if}
        </label>
      {/if}
      {#if info.musicFiles > 0}
        <label class="check">
          <input type="checkbox" bind:checked={addExisting} />
          <span>{tn(info.musicFiles, "Add the song already on the iPod", "Add the {n} songs already on the iPod")}</span>
        </label>
      {/if}
      {#if rebuild || !info.isDisk}
        <label class="check">
          <input type="checkbox" bind:checked={confirmed} />
          <span>{rebuild ? t("Yes, replace this iPod's database (playlists may be lost)") : t("Yes, this is an iPod's drive")}</span>
        </label>
      {/if}
    </div>
  {/if}
  {#snippet buttons()}
    <button onclick={close}>{t("Cancel")}</button>
    <button class="primary" disabled={blocked || !!store.busy} onclick={go}>{rebuild ? t("Rebuild") : t("Set Up")}</button>
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
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
  }
  label small {
    font-weight: 400;
    line-height: 1.4;
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
