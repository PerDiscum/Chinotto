<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import { BROWSE_FIELDS, type BrowserColumn } from "./library";
  import { count } from "./format";

  let { columns }: { columns: BrowserColumn[] } = $props();

  const cols = $derived(
    columns.map((c) => {
      const meta = BROWSE_FIELDS.find((f) => f.field === c.field)!;
      return {
        title: meta.title,
        plural: meta.plural,
        items: c.items,
        value: store.browserSel[c.field] ?? null,
        pick: (v: string | null) => store.pick(c.field, v),
      };
    }),
  );

  /** Right-click a column title: choose which columns show. */
  function chooseColumns(e: MouseEvent) {
    e.preventDefault();
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: BROWSE_FIELDS.map((f) => ({
        label: `${store.browserFields.includes(f.field) ? "✓ " : "\u2003"}${f.title}`,
        disabled: store.browserFields.includes(f.field) && store.browserFields.length === 1,
        action: () => store.toggleBrowserField(f.field),
      })),
    };
  }

  // Height of the browser, dragged with the bar under it (remembered).
  let height = $state(loadHeight());
  function loadHeight() {
    try {
      const v = Number(localStorage.getItem("chinotto.browserHeight"));
      return v >= 80 ? v : 168;
    } catch {
      return 168;
    }
  }
  function startResize(e: MouseEvent) {
    e.preventDefault();
    const y0 = e.clientY, h0 = height;
    const move = (m: MouseEvent) => (height = Math.max(80, Math.min(window.innerHeight * 0.6, h0 + m.clientY - y0)));
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      try {
        localStorage.setItem("chinotto.browserHeight", String(Math.round(height)));
      } catch {
        /* not essential */
      }
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }

  // When a list changes length (other filters picked), keep its choice in view.
  let lists: HTMLElement[] = $state([]);
  $effect(() => {
    void columns;
    queueMicrotask(() => {
      for (const l of lists) {
        const sel = l?.querySelector<HTMLElement>(".sel");
        if (sel && sel !== l.firstElementChild) sel.scrollIntoView({ block: "nearest" });
        else l?.scrollTo({ top: 0 });
      }
    });
  });

  function keydown(e: KeyboardEvent, items: string[], value: string | null, pick: (v: string | null) => void) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = value === null ? -1 : items.indexOf(value);
    const n = e.key === "ArrowDown" ? Math.min(items.length - 1, i + 1) : Math.max(-1, i - 1);
    pick(n < 0 ? null : items[n]);
    const list = e.currentTarget as HTMLElement;
    queueMicrotask(() => list.querySelector<HTMLElement>(".sel")?.scrollIntoView({ block: "nearest" }));
  }
</script>

<div class="browser" style:height="{height}px">
  {#each cols as c, ci (c.title)}
    <section>
      <h3 oncontextmenu={chooseColumns}>{c.title}</h3>
      <div class="list" bind:this={lists[ci]} role="listbox" tabindex="0" aria-label={c.title} onkeydown={(e) => keydown(e, c.items, c.value, c.pick)}>
        <button role="option" aria-selected={c.value === null} class:sel={c.value === null} onclick={() => c.pick(null)}>
          {t("All ({n} {what})", { n: c.items.length, what: c.items.length === 1 ? c.plural[0] : c.plural[1] })}
        </button>
        {#each c.items as item (item)}
          <button role="option" aria-selected={c.value === item} class:sel={c.value === item} onclick={() => c.pick(item)}>{item}</button>
        {/each}
      </div>
    </section>
  {/each}
</div>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="resize" role="separator" aria-orientation="horizontal" aria-label={t("Resize the column browser")} onmousedown={startResize}></div>

<style>
  .browser {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    flex: none;
  }
  .resize {
    flex: none;
    height: 5px;
    margin-top: -2px;
    border-bottom: 1px solid var(--edge);
    cursor: row-resize;
  }
  section {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--edge);
  }
  section:last-child {
    border-right: 0;
  }
  h3 {
    margin: 0;
    padding: 3px 8px;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    background: var(--header);
    border-bottom: 1px solid var(--edge);
    text-align: center;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    background: var(--bg);
  }
  .list:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .list button {
    display: block;
    width: 100%;
    height: 19px;
    padding: 0 8px;
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: 12px;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }
  .list button:nth-child(even) {
    background: var(--zebra);
  }
  .list button.sel {
    background: var(--sel-quiet);
  }
  .list:focus-within button.sel {
    background: var(--sel);
    color: var(--sel-ink);
  }
</style>
