<script lang="ts">
  import { t, tn } from "./i18n";
  import { store, isMac } from "./store.svelte";
  import { bytes, duration } from "./format";
  import { podcastCoverUrl } from "./api";
  import Icon from "./Icon.svelte";

  // A podcast's page: the show, and its episodes to download and play.
  // With id "" (or before setting up), the Podcasts page itself.
  let { id }: { id: string } = $props();

  const pods = $derived(store.podcasts);
  const show = $derived(pods?.shows.find((s) => s.id === id) ?? null);
  let open = $state<string | null>(null);
  let more = $state(false);

  const dateFmt = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });
  const date = (secs: number) => (secs ? dateFmt.format(new Date(secs * 1000)) : "");
  const ago = (secs: number) => {
    const m = Math.round((Date.now() / 1000 - secs) / 60);
    return m < 1 ? t("just now") : m < 60 ? tn(m, "{n} minute ago", "{n} minutes ago") : m < 1440 ? tn(Math.round(m / 60), "{n} hour ago", "{n} hours ago") : date(secs);
  };
  const newCount = $derived(show ? store.newEpisodes(show) : 0);
  // The open iPod: which of this show's episodes it has, and which were played there.
  const ipod = $derived(store.ipod);
  $effect(() => {
    void ipod?.generation;
    void ipod?.path;
    void store.refreshIpodPodcasts();
  });
  const onIpod = $derived(new Map(store.podOnIpod.filter((o) => o.show === id).map((o) => [o.key, o])));
  const sendable = $derived(show ? show.episodes.filter((e, i) => show.downloaded[i] && !onIpod.has(e.key)).map((e) => e.key) : []);
  const playedThere = $derived([...onIpod.values()].filter((o) => o.played).length);
  const canSend = $derived(!!ipod?.writable && !store.podIpodError);

  function episodeMenu(ev: MouseEvent, key: string, i: number) {
    if (!show) return;
    ev.preventDefault();
    const e = show.episodes[i];
    const have = show.downloaded[i];
    const there = onIpod.get(key);
    store.menu = {
      x: ev.clientX,
      y: ev.clientY,
      items: [
        ...(have
          ? [
              { label: t("Play"), action: () => store.playEpisode(show, key) },
              { label: isMac ? t("Show in Finder") : t("Show in Folder"), action: () => store.showInFinder(`pod:${show.id}:${key}`) },
            ]
          : [{ label: t("Download"), action: () => store.downloadEpisode(show.id, key) }]),
        ...(ipod && have && !there ? [{ label: t("Copy to {name}", { name: ipod.name }), disabled: !canSend, action: () => store.sendEpisodes(show.id, [key]) }] : []),
        ...(ipod && there ? [{ label: t("Remove from {name}", { name: ipod.name }), disabled: !canSend, action: () => store.removeEpisodesFromIpod([there.dbid]) }] : []),
        { label: "", separator: true },
        e.played ? { label: t("Mark as Unplayed"), action: () => store.markEpisodes(show.id, [key], { played: false }) } : { label: t("Mark as Played"), action: () => store.markEpisodes(show.id, [key], { played: true, new: false }) },
        ...(have ? [{ label: t("Delete Download"), danger: true, action: () => store.deleteEpisodeFile(show.id, key) }] : []),
      ],
    };
  }
  const playingKey = $derived(store.nowPlaying?.startsWith(`pod:${id}:`) ? store.nowPlaying.split(":")[2] : null);
</script>

<div class="page">
  {#if !pods?.folder}
    <div class="intro">
      <h1>{t("Podcasts")}</h1>
      <p>{t("Follow your podcasts by their feed: Chinotto checks for new episodes each time it starts, lists them, and downloads the ones you choose. On an iPod they go under Podcasts, not Music.")}</p>
      <p>{t("First, choose a folder for the episodes. Then paste a podcast's feed address (or its Apple Podcasts link).")}</p>
      <div class="controls">
        <button class="primary" onclick={() => store.setUpPodcasts()}>{t("Choose Folder…")}</button>
      </div>
    </div>
  {:else if !show}
    <div class="intro">
      <h1>{t("Podcasts")}</h1>
      <p>{pods.shows.length ? t("Choose a podcast in the sidebar, or follow another.") : t("You're not following any podcasts yet.")}</p>
      <p class="muted small">{t("Episodes are saved in {folder}", { folder: pods.folder })}</p>
      <div class="controls">
        <button class="primary" onclick={() => (store.dialog = { kind: "podcast-add" })}>{t("Add Podcast…")}</button>
        <button onclick={() => store.setUpPodcasts()}>{t("Change Folder…")}</button>
      </div>
    </div>
  {:else}
    <header class="show">
      {#if show.hasCover}
        <img class="cover" src={podcastCoverUrl(show.id, 400, show.checked)} alt="" />
      {:else}
        <div class="cover none"><Icon name="podcast" size={56} /></div>
      {/if}
      <div class="about">
        <h1>{show.title}</h1>
        {#if show.author}<p class="author">{show.author}</p>{/if}
        {#if show.description}
          <p class="desc" class:more>{show.description}</p>
          {#if show.description.length > 260}<button class="link" onclick={() => (more = !more)}>{more ? t("Less") : t("More")}</button>{/if}
        {/if}
        <p class="muted small">
          {tn(show.episodes.length, "{n} episode", "{n} episodes")} · {t("checked {when}", { when: ago(show.checked) })}
        </p>
        <div class="controls">
          <button onclick={() => store.refreshPodcasts(show.id)} disabled={store.podRefreshing}>{store.podRefreshing ? t("Checking…") : t("Check for New Episodes")}</button>
          {#if newCount}<button onclick={() => store.markEpisodes(show.id, [], { new: false })}>{t("Mark All as Seen")}</button>{/if}
          <button onclick={() => (store.dialog = { kind: "podcast-remove", id: show.id, name: show.title, downloaded: show.downloaded.filter(Boolean).length })}>{t("Unfollow…")}</button>
        </div>
        {#if ipod}
          <div class="ipod">
            {#if store.podIpodError}
              <p class="muted small">{store.podIpodError}</p>
            {:else}
              <p class="small">
                <strong>{ipod.name}:</strong>
                {tn(onIpod.size, "{n} episode", "{n} episodes")}{#if playedThere}, {tn(playedThere, "{n} played", "{n} played")}{/if}
                {#if !ipod.writable}<span class="muted"> · {t("Turn on “Allow changes” on the iPod's page to copy episodes.")}</span>{/if}
              </p>
              <div class="controls">
                <button class="primary" disabled={!canSend || !sendable.length} onclick={() => store.sendEpisodes(show.id, sendable)}>
                  {sendable.length ? tn(sendable.length, "Copy {n} Downloaded Episode to iPod", "Copy {n} Downloaded Episodes to iPod") : t("All Downloads Are on the iPod")}
                </button>
                {#if playedThere}
                  <button disabled={!canSend} onclick={() => (store.dialog = { kind: "podcast-remove-played", id: show.id, name: show.title, count: playedThere })}>{t("Remove Played…")}</button>
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </header>

    <ol class="episodes">
      {#each show.episodes as e, i (e.key)}
        {@const have = show.downloaded[i]}
        {@const prog = store.podProgress[`${show.id}:${e.key}`]}
        {@const there = onIpod.get(e.key)}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <li class:open={open === e.key} class:playing={playingKey === e.key} oncontextmenu={(ev) => episodeMenu(ev, e.key, i)}>
          <div class="ep">
            <span class="dot" class:new={e.new && !have} class:unplayed={have && !e.played} title={e.new && !have ? t("New") : have && !e.played ? t("Not played yet") : ""}></span>
            <button class="title" onclick={() => (open = open === e.key ? null : e.key)} title={t("Show or hide the description")}>
              <strong>{e.title}</strong>
              {#if e.season || (e.number && !e.title.includes(String(e.number)))}<span class="muted">{e.season ? t("S{s} E{e}", { s: e.season, e: e.number }) : t("Episode {e}", { e: e.number })}</span>{/if}
            </button>
            <span class="muted when">{date(e.published)}</span>
            <span class="muted len">{e.durationMs ? duration(e.durationMs) : ""}</span>
            <span class="act">
              {#if prog}
                <span class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={prog.total ? Math.round((prog.done / prog.total) * 100) : 0}>
                  <span style:width="{prog.total ? (prog.done / prog.total) * 100 : 3}%"></span>
                </span>
                <button class="quiet" title={t("Stop downloading")} onclick={() => store.cancelDownload(show.id, e.key)}>{t("Cancel")}</button>
              {:else if have}
                {#if there}
                  <span class="tag" title={there.played ? t("Played on the iPod") : t("On the iPod")}>{there.played ? t("Played on iPod") : t("On iPod")}</span>
                {:else if ipod && !store.podIpodError}
                  <button disabled={!canSend} onclick={() => store.sendEpisodes(show.id, [e.key])}>{t("Copy to iPod")}</button>
                {/if}
                <button onclick={() => store.playEpisode(show, e.key)}>{t("Play")}</button>
                <button class="quiet" title={t("Delete the downloaded file")} onclick={() => store.deleteEpisodeFile(show.id, e.key)}>{t("Delete")}</button>
              {:else}
                <button onclick={() => store.downloadEpisode(show.id, e.key)}>{e.bytes ? t("Download ({size})", { size: bytes(e.bytes) }) : t("Download")}</button>
                {#if e.new}<button class="quiet" title={t("No longer count this as new")} onclick={() => store.markEpisodes(show.id, [e.key], { new: false })}>{t("Dismiss")}</button>{/if}
              {/if}
            </span>
          </div>
          {#if open === e.key && e.description}
            <p class="notes">{e.description}</p>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .page {
    flex: 1;
    overflow: auto;
    padding: 28px 36px;
    background: var(--bg);
  }
  .intro {
    max-width: 560px;
  }
  h1 {
    margin: 0 0 8px;
    font-size: 24px;
  }
  p {
    line-height: 1.5;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 12px 0 4px;
  }
  .controls button {
    font: inherit;
    font-size: 13px;
    padding: 4px 12px;
    border: 1px solid var(--edge);
    border-radius: 6px;
    background: var(--zebra);
    color: var(--ink);
  }
  .controls button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .controls button:disabled {
    opacity: 0.5;
  }
  .show {
    display: flex;
    gap: 24px;
    align-items: flex-start;
    margin-bottom: 20px;
  }
  .cover {
    flex: none;
    width: 180px;
    height: 180px;
    border-radius: 8px;
    object-fit: cover;
    box-shadow: 0 2px 10px rgb(0 0 0 / 0.25);
  }
  .cover.none {
    display: grid;
    place-items: center;
    background: var(--zebra);
    color: var(--muted);
  }
  .about {
    min-width: 0;
    max-width: 720px;
  }
  .author {
    margin: 0 0 8px;
    font-weight: 600;
    color: var(--muted);
  }
  .desc {
    margin: 0;
    white-space: pre-line;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .desc.more {
    display: block;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: 12px;
  }
  .episodes {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--edge);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg);
  }
  .episodes li {
    border-bottom: 1px solid var(--edge);
  }
  .episodes li:last-child {
    border-bottom: 0;
  }
  .episodes li:nth-child(even) {
    background: var(--zebra);
  }
  .episodes li.playing .title strong {
    color: var(--accent);
  }
  .ep {
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) 96px 64px max-content;
    align-items: center;
    gap: 10px;
    min-height: 34px;
    padding: 4px 10px;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .dot.new {
    background: var(--accent-bright);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .dot.unplayed {
    box-shadow: inset 0 0 0 2px var(--accent-bright);
  }
  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    text-align: left;
  }
  .title strong {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-weight: 600;
  }
  .title span {
    font-size: 11px;
  }
  .when,
  .len {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .act {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    min-width: 150px;
  }
  .act button {
    font: inherit;
    font-size: 12px;
    padding: 2px 10px;
    border: 1px solid var(--edge);
    border-radius: 5px;
    background: var(--bg);
    color: var(--ink);
  }
  .act button.quiet {
    border-color: transparent;
    background: none;
    color: var(--muted);
  }
  .tag {
    align-self: center;
    padding: 1px 8px;
    border-radius: 9px;
    background: var(--sel-quiet);
    color: var(--ink);
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
  }
  .ipod {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--edge);
  }
  .ipod p {
    margin: 0;
  }
  .bar {
    display: block;
    width: 140px;
    height: 6px;
    margin: 6px 0;
    border-radius: 3px;
    background: var(--edge);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .notes {
    margin: 0;
    padding: 0 16px 12px 42px;
    font-size: 12px;
    white-space: pre-line;
    color: var(--muted);
    user-select: text;
    -webkit-user-select: text;
  }
  @media (max-width: 760px) {
    .show {
      flex-direction: column;
    }
    .ep {
      grid-template-columns: 12px minmax(0, 1fr) max-content;
    }
    .when,
    .len {
      display: none;
    }
  }
</style>
