#!/usr/bin/env bash
# Builds cdrdao (GPL v2, unmodified) from third_party/sources, for burning
# .bin/.cue images raw. On macOS it's one universal binary (Apple Silicon and
# Intel) that the app bundle carries next to Chinotto (Tauri "externalBin").
#
#   scripts/build-cdrdao.sh [output folder]    (default: app/src-tauri/binaries)
#
# Output: <folder>/cdrdao-<target triple> (macOS: cdrdao-universal-apple-darwin).
# Needs: Xcode command line tools (macOS) or a C++ compiler (Linux). Nothing
# optional (MP3, Ogg, the GUI) is built, so it links only system libraries.
set -euo pipefail

VERSION=1.2.6
SHA256=0cf29e1183ffd8e4d167c403d7a6ea2109a2dd4cd97e61f80417cf1373220ff4
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARBALL="$ROOT/third_party/sources/cdrdao-$VERSION.tar.bz2"
OUT="$(mkdir -p "${1:-$ROOT/app/src-tauri/binaries}" && cd "${1:-$ROOT/app/src-tauri/binaries}" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

sum=$( (command -v sha256sum >/dev/null && sha256sum "$TARBALL" || shasum -a 256 "$TARBALL") | cut -d' ' -f1)
[ "$sum" = "$SHA256" ] || { echo "cdrdao source doesn't match its checksum" >&2; exit 1; }
tar -xjf "$TARBALL" -C "$WORK"
cd "$WORK/cdrdao-$VERSION"

flags=""
if [ "$(uname)" = Darwin ]; then
  export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
  flags="-arch arm64 -arch x86_64 -mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET"
  triple=universal-apple-darwin
else
  triple="$(rustc -vV | sed -n 's/^host: //p')"
fi
CFLAGS="-O2 $flags" CXXFLAGS="-O2 $flags" LDFLAGS="$flags" ./configure \
  --without-gcdmaster --without-ogg-support --without-mp3-support --without-lame \
  --disable-dependency-tracking >"$WORK/build.log" 2>&1 || { tail -40 "$WORK/build.log"; exit 1; }
make -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)" >>"$WORK/build.log" 2>&1 || { tail -40 "$WORK/build.log"; exit 1; }
strip dao/cdrdao 2>/dev/null || true
cp dao/cdrdao "$OUT/cdrdao-$triple"
cp COPYING "$OUT/cdrdao-COPYING"
echo "Built $OUT/cdrdao-$triple"
