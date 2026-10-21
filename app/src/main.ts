import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
import { initLanguage } from "./lib/i18n";

// The language is chosen before the app (and its texts) load.
(async () => {
  const system = await invoke<string[]>("system_locales").catch(() => [] as string[]);
  initLanguage(system);
  const { default: App } = await import("./App.svelte");
  mount(App, { target: document.getElementById("app")! });
})();
