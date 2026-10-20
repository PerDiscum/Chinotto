import type { Track } from "./types";

const nf = new Intl.NumberFormat();
export const count = (n: number) => nf.format(n);

export function duration(ms: number): string {
  const s = Math.round(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** Total time the way iTunes' status bar says it: "65.7 days", "3.2 hours". */
export function longDuration(ms: number): string {
  const hours = ms / 3_600_000;
  if (hours >= 24) return `${(hours / 24).toFixed(1)} days`;
  if (hours >= 1) return `${hours.toFixed(1)} hours`;
  return `${Math.round(ms / 60000)} minutes`;
}

export function bytes(n: number): string {
  if (n >= 1e12) return `${(n / 1e12).toFixed(2)} TB`;
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  if (n >= 1e3) return `${Math.round(n / 1e3)} KB`;
  return `${n} bytes`;
}

export const stars = (r: number) => "★".repeat(r);

export const date = (unix?: number) =>
  unix ? new Date(unix * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }) : "";

export const isVideo = (t: Track) => (t.mediaType & (0x02 | 0x20 | 0x40)) !== 0;

export const UNKNOWN_ARTIST = "Unknown Artist";
export const UNKNOWN_ALBUM = "Unknown Album";
export const UNKNOWN_GENRE = "Unknown Genre";
export const COMPILATIONS = "Compilations";

/** The artist a track is filed under in the column browser (same grouping the iPod uses). */
export const browseArtist = (t: Track) =>
  t.compilation ? COMPILATIONS : t.albumArtist || t.artist || UNKNOWN_ARTIST;
export const browseAlbum = (t: Track) => t.album || UNKNOWN_ALBUM;
export const browseGenre = (t: Track) => t.genre || UNKNOWN_GENRE;

/** Text used for sorting when no explicit sort field exists: drop a leading "The ". */
export const stripThe = (s: string) => s.replace(/^the\s+/i, "");

/** Accent- and case-insensitive text for searching. */
export const fold = (s: string) =>
  s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
