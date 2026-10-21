#!/usr/bin/env bash
# Builds a static glib for macOS (Apple Silicon and Intel) for libgpod to link
# against, so Chinotto.app needs no extra libraries.
#
#   scripts/build-glib-macos.sh [output folder]      (default: ./deps)
#
# Output: <folder>/aarch64-apple-darwin and <folder>/x86_64-apple-darwin, each a
# prefix with lib/*.a and lib/pkgconfig. Point CHINOTTO_DEPS at <folder>.
# Needs: Xcode command line tools, meson, ninja (brew install meson ninja).
# Everything glib depends on (pcre2, libffi, libintl, zlib) is built from
# source too (--wrap-mode=forcefallback), so nothing from Homebrew is linked in.
set -euo pipefail

GLIB_VERSION="${GLIB_VERSION:-2.82.5}"
MIN_MACOS="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
OUT="$(mkdir -p "${1:-deps}" && cd "${1:-deps}" && pwd)"
WORK="$OUT/.build"
mkdir -p "$WORK"

series="${GLIB_VERSION%.*}"
tarball="$WORK/glib-$GLIB_VERSION.tar.xz"
if [ ! -f "$tarball" ]; then
  curl -fL --retry 3 -o "$tarball" "https://download.gnome.org/sources/glib/$series/glib-$GLIB_VERSION.tar.xz"
fi
echo "glib-$GLIB_VERSION.tar.xz sha256: $(shasum -a 256 "$tarball" | cut -d' ' -f1)"
rm -rf "$WORK/glib-$GLIB_VERSION"
tar -xf "$tarball" -C "$WORK"
src="$WORK/glib-$GLIB_VERSION"

for triple in aarch64-apple-darwin x86_64-apple-darwin; do
  case "$triple" in
    aarch64-*) arch=arm64;  cpu=aarch64 ;;
    x86_64-*)  arch=x86_64; cpu=x86_64 ;;
  esac
  prefix="$OUT/$triple"
  if [ -f "$prefix/lib/pkgconfig/gobject-2.0.pc" ]; then
    echo "glib for $triple already built"
    continue
  fi
  cross="$WORK/cross-$arch.ini"
  # -Dfdopen=fdopen: glib pins zlib 1.2.11, whose zutil.h turns fdopen() into
  # NULL on macOS; newer Xcode's stdio.h then fails to compile ("expected
  # identifier"). Defining fdopen as itself skips that (zlib 1.2.12 fixed it).
  cat > "$cross" <<INI
[binaries]
c = 'clang'
objc = 'clang'
cpp = 'clang++'
ar = 'ar'
strip = 'strip'

[built-in options]
c_args = ['-arch', '$arch', '-mmacosx-version-min=$MIN_MACOS', '-Dfdopen=fdopen']
objc_args = ['-arch', '$arch', '-mmacosx-version-min=$MIN_MACOS', '-Dfdopen=fdopen']
cpp_args = ['-arch', '$arch', '-mmacosx-version-min=$MIN_MACOS']
c_link_args = ['-arch', '$arch', '-mmacosx-version-min=$MIN_MACOS']
objc_link_args = ['-arch', '$arch', '-mmacosx-version-min=$MIN_MACOS']
cpp_link_args = ['-arch', '$arch', '-mmacosx-version-min=$MIN_MACOS']

[host_machine]
system = 'darwin'
cpu_family = '$cpu'
cpu = '$cpu'
endian = 'little'
INI
  build="$WORK/build-$arch"
  rm -rf "$build"
  meson setup "$build" "$src" \
    --cross-file "$cross" \
    --prefix "$prefix" \
    --buildtype release \
    --default-library static \
    --wrap-mode forcefallback \
    -Dtests=false \
    -Dintrospection=disabled \
    -Dnls=disabled \
    -Dman-pages=disabled \
    -Ddocumentation=false \
    -Dsysprof=disabled \
    -Dlibelf=disabled \
    -Dglib_debug=disabled
  meson compile -C "$build"
  meson install -C "$build"
  # glib's helper libraries (pcre2, libffi, libintl, zlib) are built as meson
  # subprojects; make sure their static libraries sit next to glib's.
  find "$build/subprojects" -name 'lib*.a' -exec cp -n {} "$prefix/lib/" \;
  # proxy-libintl installs its library but not its header, which glib's
  # gi18n-lib.h (used by libgpod) includes. It must be this header: it maps
  # gettext() etc. onto the library's g_libintl_* names.
  if [ ! -f "$prefix/include/libintl.h" ]; then
    hdr="$(find "$src/subprojects" "$build/subprojects" -name libintl.h -print -quit)"
    [ -n "$hdr" ] || { echo "libintl.h not found in glib's subprojects" >&2; exit 1; }
    cp "$hdr" "$prefix/include/"
  fi
  ls "$prefix/lib"/*.a
  echo "glib for $triple installed in $prefix"
done

echo
echo "Done. Set CHINOTTO_DEPS=$OUT"
