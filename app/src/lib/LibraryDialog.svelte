<script lang="ts">
  import { untrack } from "svelte";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import { pickMusicFolders } from "./api";

  let { id }: { id: string | null } = $props();

  const existing = untrack(() => (id ? store.libraries.find((l) => l.id === id) : undefined));
  let name = $state(existing?.name ?? (store.libraries.length ? "" : "Music"));
  let folders = $state<string[]>(existing?.folders.map((f) => f.path) ?? []);
  const available = (p: string) => existing?.folders.find((f) => f.path === p)?.available ?? true;
  let saving = $state(false);

  async function addFolders() {
    const picked = await pickMusicFolders();
    folders = [...new Set([...folders, ...picked])];
  }

  async function save() {
    if (!name.trim() || !folders.length || saving) return;
    saving = true;
    if (id) await store.updateLibrary(id, name.trim(), folders);
    else await store.createLibrary(name.trim(), folders);
    saving = false;
  }

  const close = () => (store.dialog = null);
</script>

<Modal title={id ? "Edit Library" : "New Library"} onclose={close} width={520}>
  <div class="form">
    <label>
      <span>Name</span>
      <input bind:value={name} placeholder="e.g. Lossless Archive" onkeydown={(e) => e.key === "Enter" && save()} />
    </label>
    <div class="folders">
      <span>Music folders</span>
      {#if folders.length}
        <ul>
          {#each folders as f (f)}
            <li class:off={!available(f)}>
              <span class="path" title={f}>{f}</span>
              {#if !available(f)}<span class="note">not connected</span>{/if}
              <button class="remove" aria-label="Take {f} out of the library" onclick={() => (folders = folders.filter((x) => x !== f))}>×</button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="empty">No folders yet.</p>
      {/if}
      <button onclick={addFolders}>Add Folder…</button>
    </div>
    <p class="hint">
      Chinotto lists the music in these folders and keeps the list up to date (every hour, or with Scan Now). Your music
      files are never changed. Songs on a drive that isn't connected stay in the library until it's back.
    </p>
  </div>
  {#snippet buttons()}
    <button onclick={close}>Cancel</button>
    <button class="primary" onclick={save} disabled={!name.trim() || !folders.length || saving}>{id ? "OK" : "Create Library"}</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  label,
  .folders {
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
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 160px;
    overflow-y: auto;
    border: 1px solid var(--edge);
    border-radius: 6px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
    font-weight: 400;
  }
  li:nth-child(even) {
    background: var(--zebra);
  }
  li.off .path {
    color: var(--muted);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }
  .note {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
  .remove {
    flex: none;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 15px;
  }
  .folders > button {
    align-self: flex-start;
  }
  .empty {
    margin: 0;
    font-weight: 400;
    color: var(--muted);
  }
  .hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }
</style>
