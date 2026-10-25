<script lang="ts">
  import { t } from "./i18n";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";

  // Follow a podcast by its feed address (or Apple Podcasts link).
  let url = $state("");
  let error = $state<string | null>(null);
  let working = $state(false);
  const close = () => (store.dialog = null);

  async function add() {
    if (working || !url.trim()) return;
    working = true;
    error = await store.addPodcast(url);
    working = false;
    if (!error) close();
  }
</script>

<Modal title={t("Add a Podcast")} onclose={close} width={520}>
  <label>
    <span>{t("Feed address")}</span>
    <input bind:value={url} placeholder="https://example.com/podcast.xml" disabled={working} spellcheck="false" autocomplete="off" onkeydown={(e) => e.key === "Enter" && add()} />
  </label>
  <p class="hint">{t("Paste the show's RSS feed address, usually found on its website as “RSS” or “Subscribe”. An Apple Podcasts link works too.")}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#snippet buttons()}
    <button onclick={close}>{t("Cancel")}</button>
    <button class="primary" onclick={add} disabled={working || !url.trim()}>{working ? t("Reading the feed…") : t("Add Podcast")}</button>
  {/snippet}
</Modal>

<style>
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
  }
  input {
    font: inherit;
    font-weight: 400;
    font-size: 13px;
    padding: 4px 6px;
  }
  .hint {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }
  .error {
    margin: 10px 0 0;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 12px;
  }
</style>
