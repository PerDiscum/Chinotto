<script lang="ts">
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import GetInfo from "./GetInfo.svelte";
  import { count } from "./format";

  const close = () => (store.dialog = null);
  const ipodName = $derived(store.ipod?.name ?? "the iPod");
</script>

{#if store.dialog?.kind === "info"}
  <GetInfo />
{:else if store.dialog?.kind === "delete-songs"}
  {@const ids = store.dialog.ids}
  {@const n = ids.length}
  <Modal title="Delete {n === 1 ? 'this song' : `${count(n)} songs`} from {ipodName}?" onclose={close}>
    <p>
      {n === 1 ? "It" : "They"}'ll be removed from the iPod and from all its playlists, and the music
      {n === 1 ? "file" : "files"} deleted from the iPod. Your own copies elsewhere aren't touched.
    </p>
    {#snippet buttons()}
      <button onclick={close}>Cancel</button>
      <button class="danger" onclick={() => store.deleteSongs(ids)}>Delete {n === 1 ? "Song" : "Songs"}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "delete-playlist"}
  {@const d = store.dialog}
  <Modal title="Delete the playlist “{d.name}”?" onclose={close}>
    <p>Its songs stay on the iPod.</p>
    {#snippet buttons()}
      <button onclick={close}>Cancel</button>
      <button class="danger" onclick={() => store.deletePlaylist(d.id)}>Delete Playlist</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "report"}
  {@const d = store.dialog}
  <Modal title={d.title} onclose={close} width={540}>
    <ul class="report">
      {#each d.lines as line, i (i)}<li>{line}</li>{/each}
    </ul>
    {#snippet buttons()}
      <button class="primary" onclick={close}>OK</button>
    {/snippet}
  </Modal>
{/if}

<style>
  p {
    margin: 0;
    line-height: 1.5;
    max-width: 52ch;
  }
  .report {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
    line-height: 1.5;
    user-select: text;
    -webkit-user-select: text;
  }
</style>
