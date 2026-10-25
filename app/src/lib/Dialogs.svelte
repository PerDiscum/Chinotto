<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import GetInfo from "./GetInfo.svelte";
  import LibraryDialog from "./LibraryDialog.svelte";
  import SetupDialog from "./SetupDialog.svelte";
  import MscDialog from "./MscDialog.svelte";
  import PodcastDialog from "./PodcastDialog.svelte";
  import { count } from "./format";

  const close = () => (store.dialog = null);
  const ipodName = $derived(store.ipod?.name ?? t("the iPod"));
</script>

{#if store.dialog?.kind === "info"}
  <GetInfo />
{:else if store.dialog?.kind === "delete-songs"}
  {@const ids = store.dialog.ids}
  {@const n = ids.length}
  <Modal title={tn(n, "Delete this song from {name}?", "Delete {n} songs from {name}?", { name: ipodName })} onclose={close}>
    <p>
      {tn(n, "It'll be removed from the iPod and from all its playlists, and the music file deleted from the iPod. Your own copies elsewhere aren't touched.", "They'll be removed from the iPod and from all its playlists, and the music files deleted from the iPod. Your own copies elsewhere aren't touched.")}
    </p>
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      <button class="danger" onclick={() => store.deleteSongs(ids)}>{tn(n, "Delete Song", "Delete Songs")}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "delete-playlist"}
  {@const d = store.dialog}
  <Modal title={t("Delete the playlist “{name}”?", { name: d.name })} onclose={close}>
    <p>{d.id.startsWith("lib:") ? t("Its songs stay in the library.") : t("Its songs stay on the iPod.")}</p>
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      <button class="danger" onclick={() => store.deletePlaylist(d.id)}>{t("Delete Playlist")}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "setup"}
  <SetupDialog path={store.dialog.path} rebuild={store.dialog.rebuild} />
{:else if store.dialog?.kind === "msc"}
  <MscDialog path={store.dialog.path} />
{:else if store.dialog?.kind === "podcast-add"}
  <PodcastDialog />
{:else if store.dialog?.kind === "podcast-remove"}
  {@const d = store.dialog}
  <Modal title={t("Unfollow “{name}”?", { name: d.name })} onclose={close}>
    <p>{t("Chinotto stops checking this podcast for new episodes.")}</p>
    {#if d.downloaded}
      <p>{tn(d.downloaded, "You have {n} downloaded episode. Keep it in your podcast folder, or delete it?", "You have {n} downloaded episodes. Keep them in your podcast folder, or delete them?")}</p>
    {/if}
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      {#if d.downloaded}
        <button class="danger" onclick={() => store.removePodcast(d.id, true)}>{t("Unfollow and Delete")}</button>
        <button class="primary" onclick={() => store.removePodcast(d.id, false)}>{t("Unfollow, Keep Files")}</button>
      {:else}
        <button class="primary" onclick={() => store.removePodcast(d.id, false)}>{t("Unfollow")}</button>
      {/if}
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "podcast-remove-played"}
  {@const d = store.dialog}
  <Modal title={tn(d.count, "Remove {n} played episode of “{name}”?", "Remove {n} played episodes of “{name}”?", { name: d.name })} onclose={close}>
    <p>{t("Take them off the iPod only, or delete the downloaded copies on this computer as well?")}</p>
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      <button class="danger" onclick={() => store.removePlayedEpisodes(d.id, true)}>{t("From iPod and Computer")}</button>
      <button class="primary" onclick={() => store.removePlayedEpisodes(d.id, false)}>{t("From iPod Only")}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "library"}
  <LibraryDialog id={store.dialog.id} />
{:else if store.dialog?.kind === "delete-library"}
  {@const d = store.dialog}
  <Modal title={t("Delete the library “{name}”?", { name: d.name })} onclose={close}>
    <p>{t("Chinotto forgets this library and its list of songs. Your music files and folders aren't touched.")}</p>
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      <button class="danger" onclick={() => store.deleteLibrary(d.id)}>{t("Delete Library")}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "remove-from-library"}
  {@const ids = store.dialog.ids}
  {@const n = ids.length}
  <Modal title={tn(n, "Remove this song from the library?", "Remove {n} songs from the library?")} onclose={close}>
    <p>
      {tn(n, "It'll be taken out of this library and its playlists, and won't come back at the next scan. The music file stays where it is: nothing is deleted.", "They'll be taken out of this library and its playlists, and won't come back at the next scan. The music files stay where they are: nothing is deleted.")}
    </p>
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      <button class="danger" onclick={() => store.removeFromLibrary(ids)}>{t("Remove")}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "delete-player-songs"}
  {@const d = store.dialog}
  {@const n = d.ids.length}
  {@const name = store.devices.find((x) => x.path === d.path)?.volume ?? ""}
  <Modal title={tn(n, "Delete this song from {name}?", "Delete {n} songs from {name}?", { name })} onclose={close}>
    <p>{tn(n, "Its music file is deleted from the player. Your own copies elsewhere aren't touched.", "Their music files are deleted from the player. Your own copies elsewhere aren't touched.")}</p>
    {#snippet buttons()}
      <button onclick={close}>{t("Cancel")}</button>
      <button class="danger" onclick={() => store.deleteFromPlayer(d.path, d.ids)}>{tn(n, "Delete Song", "Delete Songs")}</button>
    {/snippet}
  </Modal>
{:else if store.dialog?.kind === "report"}
  {@const d = store.dialog}
  <Modal title={d.title} onclose={close} width={540}>
    <ul class="report">
      {#each d.lines as line, i (i)}<li>{line}</li>{/each}
    </ul>
    {#snippet buttons()}
      <button class="primary" onclick={close}>{t("OK")}</button>
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
