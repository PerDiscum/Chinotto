// Collects every t("…") / tn(n, "…", "…") text in src/ into src/locales/en.json
// (English → ""), and adds new texts to the other language files, keeping
// existing translations. Run: npm run strings
import { readFileSync, writeFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join } from "node:path";

const root = new URL("../src/", import.meta.url).pathname;
const files = [];
(function walk(d) {
  for (const f of readdirSync(d)) {
    const p = join(d, f);
    if (statSync(p).isDirectory()) walk(p);
    else if (/\.(svelte|ts)$/.test(f) && !f.endsWith("i18n.ts")) files.push(p);
  }
})(root);

const found = new Set();
const str = String.raw`"((?:[^"\\]|\\.)*)"`;
const call = new RegExp(String.raw`\btn?\(\s*(?:[^,()"]+,\s*)?` + str + String.raw`(?:\s*,\s*` + str + ")?", "g");
for (const f of files) {
  const src = readFileSync(f, "utf8");
  for (const m of src.matchAll(call)) {
    for (const s of [m[1], m[2]]) if (s !== undefined) found.add(JSON.parse(`"${s}"`));
  }
}
const keys = [...found].sort((a, b) => a.localeCompare(b));
const dir = join(root, "locales");
writeFileSync(join(dir, "en.json"), JSON.stringify(Object.fromEntries(keys.map((k) => [k, ""])), null, 2) + "\n");
for (const f of readdirSync(dir).filter((f) => f.endsWith(".json") && f !== "en.json")) {
  const p = join(dir, f);
  const old = existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : {};
  const next = Object.fromEntries(keys.map((k) => [k, old[k] ?? ""]));
  const gone = Object.keys(old).filter((k) => !found.has(k) && old[k]);
  // Keep translations of texts no longer used, at the end, in case they come back.
  for (const k of gone) next[k] = old[k];
  writeFileSync(p, JSON.stringify(next, null, 2) + "\n");
}
console.log(`${keys.length} texts`);
