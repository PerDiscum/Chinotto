import type { Track } from "./types";
import { lang, t, tn } from "./i18n";

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
  const one = (n: number) => new Intl.NumberFormat(lang, { maximumFractionDigits: 1, minimumFractionDigits: 1 }).format(n);
  if (hours >= 24) return t("{x} days", { x: one(hours / 24) });
  if (hours >= 1) return t("{x} hours", { x: one(hours) });
  const m = Math.round(ms / 60000);
  return tn(m, "{n} minute", "{n} minutes");
}

export function bytes(n: number): string {
  if (n >= 1e12) return `${(n / 1e12).toFixed(2)} TB`;
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  if (n >= 1e3) return `${Math.round(n / 1e3)} KB`;
  return t("{n} bytes", { n });
}

export const stars = (r: number) => "★".repeat(r);

export const date = (unix?: number) =>
  unix ? new Date(unix * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }) : "";

export const isVideo = (t: Track) => (t.mediaType & (0x02 | 0x20 | 0x40)) !== 0;

/** Short format name for a song: MP3, AAC, ALAC, WAV, AIFF… */
export function formatName(t: Track): string {
  const k = t.kind.toLowerCase();
  const ext = (t.location ?? "").split(".").pop()?.toLowerCase() ?? "";
  if (k.includes("lossless")) return "ALAC";
  if (k.includes("flac") || ext === "flac") return "FLAC";
  if (k.includes("vorbis") || ext === "ogg" || ext === "oga") return "Ogg";
  if (k.includes("opus") || ext === "opus") return "Opus";
  if (k.includes("windows media") || ext === "wma") return "WMA";
  if (k.includes("mpeg audio") || ext === "mp3") return "MP3";
  if (k.includes("wav") || ext === "wav") return "WAV";
  if (k.includes("aiff") || ext === "aif" || ext === "aiff") return "AIFF";
  if (k.includes("video") || ["m4v", "mp4", "mov"].includes(ext)) return "Video";
  if (k.includes("aac") || k.includes("mpeg-4") || k.includes("audiobook") || ["m4a", "m4b", "m4p", "aac"].includes(ext)) {
    // Apple Lossless isn't always labeled; its bit rate gives it away.
    return t.bitrate > 400 && ext !== "m4p" ? "ALAC" : "AAC";
  }
  return t.kind || "Unknown";
}

export const bitrate = (t: Track) => (t.bitrate ? `${t.bitrate} kbps` : "");

export const UNKNOWN_ARTIST = t("Unknown Artist");
export const UNKNOWN_ALBUM = t("Unknown Album");
export const UNKNOWN_GENRE = t("Unknown Genre");
export const COMPILATIONS = t("Compilations");
export const VARIOUS = t("Various Artists");
export const UNKNOWN_COMPOSER = t("Unknown Composer");
export const UNKNOWN_YEAR = t("Unknown Year");

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

export const browseComposer = (t: Track) => t.composer || UNKNOWN_COMPOSER;
export const browseYear = (t: Track) => (t.year ? String(t.year) : UNKNOWN_YEAR);
