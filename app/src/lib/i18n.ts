// Translations. English text is the key: t("Add Folder…") looks the text up
// in the language's file (src/locales/<lang>.json) and falls back to English.
// Words in {braces} are filled in: t("{n} songs", { n: 5 }).
//
// To translate: copy src/locales/en.json (every text in the app, made by
// `npm run strings`) to <lang>.json and fill in the right-hand sides.
// Empty or missing entries stay English.

const files = import.meta.glob<Record<string, string>>("../locales/*.json", { eager: true, import: "default" });
const catalogs: Record<string, Record<string, string>> = {};
for (const [path, data] of Object.entries(files)) catalogs[path.split("/").pop()!.replace(".json", "")] = data;

/** Languages with a translation file (besides English). */
export const languages = Object.keys(catalogs).filter((l) => l !== "en");

function pick(system: string[]): string {
  try {
    const chosen = localStorage.getItem("chinotto.language");
    if (chosen && (chosen === "en" || catalogs[chosen])) return chosen;
  } catch {
    /* not essential */
  }
  const nav = typeof navigator !== "undefined" ? [...navigator.languages] : [];
  for (const l of [...system, ...nav]) {
    const base = l.toLowerCase().split("-")[0];
    if (base === "en") return "en";
    if (catalogs[base] && Object.values(catalogs[base]).some(Boolean)) return base;
  }
  return "en";
}

/** The language in use; set once by `initLanguage` before the app starts. */
export let lang = "en";
let table: Record<string, string> = {};

/** Choose the language: the user's choice, else the system's (`system`: e.g. ["de-DE", "en-US"]). */
export function initLanguage(system: string[]) {
  lang = pick(system);
  table = lang === "en" ? {} : (catalogs[lang] ?? {});
  if (typeof document !== "undefined") document.documentElement.lang = lang;
}

function fill(s: string, vars?: Record<string, string | number>): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(typeof vars[k] === "number" ? new Intl.NumberFormat(lang).format(vars[k] as number) : vars[k]) : m));
}

/** The text in the user's language. */
export function t(english: string, vars?: Record<string, string | number>): string {
  return fill(table[english] || english, vars);
}

/** Singular or plural: tn(n, "{n} song", "{n} songs"). */
export function tn(n: number, one: string, other: string, vars?: Record<string, string | number>): string {
  return t(n === 1 ? one : other, { n, ...vars });
}

/** Use this language from the next start ("" = the system's). */
export function setLanguage(l: string) {
  try {
    if (l) localStorage.setItem("chinotto.language", l);
    else localStorage.removeItem("chinotto.language");
  } catch {
    /* not essential */
  }
}
