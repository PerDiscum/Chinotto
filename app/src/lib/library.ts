// Fast sorting and searching over an iPod's tracks.
// Strings are ranked once with a locale-aware collator so later sorts only
// compare integers (23k tracks sort in a few milliseconds).

import type { SortKey, Track } from "./types";
import { browseAlbum, browseArtist, browseGenre, fold, stripThe } from "./format";

export const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

function ranks(values: string[]): Uint32Array {
  const uniq = [...new Set(values)].sort(collator.compare);
  const m = new Map(uniq.map((v, i) => [v, i]));
  return Uint32Array.from(values, (v) => m.get(v)!);
}

export class TrackIndex {
  readonly tracks: Track[];
  readonly byId = new Map<number, Track>();
  private pos = new Map<number, number>();
  private rTitle: Uint32Array;
  private rArtist: Uint32Array;
  private rAlbum: Uint32Array;
  private rGenre: Uint32Array;
  private rKind: Uint32Array;
  private haystack: string[];

  constructor(tracks: Track[]) {
    this.tracks = tracks;
    tracks.forEach((t, i) => {
      this.byId.set(t.id, t);
      this.pos.set(t.id, i);
    });
    this.rTitle = ranks(tracks.map((t) => t.sortTitle ?? t.title ?? ""));
    this.rArtist = ranks(tracks.map((t) => t.sortArtist ?? stripThe(t.artist ?? "")));
    this.rAlbum = ranks(tracks.map((t) => t.sortAlbum ?? t.album ?? ""));
    this.rGenre = ranks(tracks.map((t) => t.genre ?? ""));
    this.rKind = ranks(tracks.map((t) => t.kind));
    this.haystack = tracks.map((t) =>
      fold([t.title, t.artist, t.albumArtist, t.album, t.composer, t.genre].filter(Boolean).join("\n")),
    );
  }

  get(ids: number[]): Track[] {
    const out: Track[] = [];
    for (const id of ids) {
      const t = this.byId.get(id);
      if (t) out.push(t);
    }
    return out;
  }

  /** Every word of the query must appear somewhere in the track's main fields. */
  search(tracks: Track[], query: string): Track[] {
    const words = fold(query).split(/\s+/).filter(Boolean);
    if (!words.length) return tracks;
    return tracks.filter((t) => {
      const h = this.haystack[this.pos.get(t.id)!];
      return words.every((w) => h.includes(w));
    });
  }

  sort(tracks: Track[], key: SortKey, dir: 1 | -1): Track[] {
    const p = (t: Track) => this.pos.get(t.id)!;
    const albumOrder = (a: Track, b: Track) =>
      this.rAlbum[p(a)] - this.rAlbum[p(b)] || a.discNr - b.discNr || a.trackNr - b.trackNr || this.rTitle[p(a)] - this.rTitle[p(b)];
    const artistOrder = (a: Track, b: Track) => this.rArtist[p(a)] - this.rArtist[p(b)] || albumOrder(a, b);
    const primary: Record<SortKey, (a: Track, b: Track) => number> = {
      title: (a, b) => this.rTitle[p(a)] - this.rTitle[p(b)] || artistOrder(a, b),
      time: (a, b) => a.lengthMs - b.lengthMs,
      artist: artistOrder,
      album: albumOrder,
      genre: (a, b) => this.rGenre[p(a)] - this.rGenre[p(b)] || artistOrder(a, b),
      year: (a, b) => a.year - b.year || artistOrder(a, b),
      rating: (a, b) => a.rating - b.rating || artistOrder(a, b),
      plays: (a, b) => a.playCount - b.playCount || artistOrder(a, b),
      added: (a, b) => (a.added ?? 0) - (b.added ?? 0),
      kind: (a, b) => this.rKind[p(a)] - this.rKind[p(b)] || artistOrder(a, b),
    };
    const cmp = primary[key];
    return [...tracks].sort((a, b) => dir * cmp(a, b));
  }
}

export interface BrowserColumns {
  genres: string[];
  artists: string[];
  albums: string[];
}

/**
 * Column browser lists (Genre → Artist → Album), each narrowed by the
 * selections to its left, plus the tracks matching all three selections.
 */
export function browse(
  tracks: Track[],
  sel: { genre: string | null; artist: string | null; album: string | null },
): { columns: BrowserColumns; tracks: Track[] } {
  const genres = new Set<string>();
  const artists = new Map<string, string>(); // name → sort key
  const albums = new Map<string, string>();
  const out: Track[] = [];
  for (const t of tracks) {
    const g = browseGenre(t);
    genres.add(g);
    if (sel.genre !== null && g !== sel.genre) continue;
    const ar = browseArtist(t);
    if (!artists.has(ar)) artists.set(ar, t.compilation ? ar : (t.albumArtist ? t.sortAlbumArtist : t.sortArtist) ?? stripThe(ar));
    if (sel.artist !== null && ar !== sel.artist) continue;
    const al = browseAlbum(t);
    if (!albums.has(al)) albums.set(al, t.sortAlbum ?? al);
    if (sel.album !== null && al !== sel.album) continue;
    out.push(t);
  }
  const byKey = (m: Map<string, string>) =>
    [...m.entries()].sort((a, b) => collator.compare(a[1], b[1])).map(([name]) => name);
  return {
    columns: { genres: [...genres].sort(collator.compare), artists: byKey(artists), albums: byKey(albums) },
    tracks: out,
  };
}
