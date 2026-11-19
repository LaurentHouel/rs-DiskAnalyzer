#!/usr/bin/env bash
set -euo pipefail

BINARY="${1:?usage: package-linux.sh <binary> <target> <tag> <output-dir>}"
TARGET="${2:?usage: package-linux.sh <binary> <target> <tag> <output-dir>}"
TAG="${3:?usage: package-linux.sh <binary> <target> <tag> <output-dir>}"
OUTPUT="${4:?usage: package-linux.sh <binary> <target> <tag> <output-dir>}"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
NAME="rs-disk-analyzer-${TAG}-${TARGET}"
APP_ID="io.github.laurenthouel.rs-disk-analyzer"
LINUXDEPLOY_VERSION="1-alpha-20251107-1"

case "$TARGET" in
  x86_64-unknown-linux-gnu)
    DEB_ARCH=amd64
    FLATPAK_ARCH=x86_64
    LINUXDEPLOY_ARCH=x86_64
    LINUXDEPLOY_SHA256=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
    ;;
  aarch64-unknown-linux-gnu)
    DEB_ARCH=arm64
    FLATPAK_ARCH=aarch64
    LINUXDEPLOY_ARCH=aarch64
    LINUXDEPLOY_SHA256=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff
    ;;
  *)
    echo "Unsupported Linux target: $TARGET" >&2
    exit 1
    ;;
esac

mkdir -p "$OUTPUT"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

tar_root="$STAGING/$NAME"
mkdir -p "$tar_root"
cp "$BINARY" "$tar_root/rs-disk-analyzer"
cp "$ROOT/README.md" "$ROOT/packaging/linux/rs-disk-analyzer.desktop" "$tar_root/"
cp "$ROOT/assets/icon-256.png" "$tar_root/rs-disk-analyzer.png"
tar -czf "$OUTPUT/$NAME.tar.gz" -C "$STAGING" "$NAME"

deb_root="$STAGING/deb"
install -D -m 755 "$BINARY" "$deb_root/usr/bin/rs-disk-analyzer"
install -D -m 644 "$ROOT/packaging/linux/rs-disk-analyzer.desktop" \
  "$deb_root/usr/share/applications/rs-disk-analyzer.desktop"
install -D -m 644 "$ROOT/assets/icon-256.png" \
  "$deb_root/usr/share/icons/hicolor/256x256/apps/rs-disk-analyzer.png"
mkdir -p "$deb_root/DEBIAN"
shlib_dependencies="$(dpkg-shlibdeps -O -e "$BINARY" | sed -n 's/^shlibs:Depends=//p')"
extra_dependencies="libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libxcb1, libxcb-randr0, libxcb-shape0, libxcb-xfixes0, libx11-6, libxcursor1, libxi6, libxrandr2, libxfixes3, libxext6, libgl1"
if [ -n "$shlib_dependencies" ]; then
  dependencies="$shlib_dependencies, $extra_dependencies"
else
  dependencies="$extra_dependencies"
fi
cat > "$deb_root/DEBIAN/control" <<CONTROL
Package: rs-disk-analyzer
Version: $VERSION
Architecture: $DEB_ARCH
Section: utils
Priority: optional
Maintainer: rs-DiskAnalyzer contributors <noreply@github.com>
Depends: $dependencies
Description: Visualize disk usage with a treemap and rings chart
CONTROL
dpkg-deb --build --root-owner-group "$deb_root" "$OUTPUT/$NAME.deb"

appimage_work="$STAGING/appimage"
mkdir -p "$appimage_work"
linuxdeploy="$STAGING/linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage"
curl --fail --location --silent --show-error \
  "https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage" \
  --output "$linuxdeploy"
echo "$LINUXDEPLOY_SHA256  $linuxdeploy" | sha256sum --check
chmod +x "$linuxdeploy"
cp "$ROOT/assets/icon-256.png" "$STAGING/rs-disk-analyzer.png"
(
  cd "$appimage_work"
  "$linuxdeploy" --appimage-extract-and-run \
    --appdir "$appimage_work/AppDir" \
    --executable "$BINARY" \
    --desktop-file "$ROOT/packaging/linux/rs-disk-analyzer.desktop" \
    --icon-file "$STAGING/rs-disk-analyzer.png" \
    --output appimage
)
appimage="$(find "$appimage_work" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
if [ -z "$appimage" ]; then
  echo "linuxdeploy did not produce an AppImage" >&2
  exit 1
fi
mv "$appimage" "$OUTPUT/$NAME.AppImage"

flatpak_input="$STAGING/build-input"
mkdir -p "$flatpak_input"
cp "$BINARY" "$flatpak_input/rs-disk-analyzer"
sed "s/^Icon=rs-disk-analyzer$/Icon=$APP_ID/" \
  "$ROOT/packaging/linux/rs-disk-analyzer.desktop" > "$flatpak_input/rs-disk-analyzer.desktop"
cp "$ROOT/assets/icon-256.png" "$flatpak_input/icon.png"
cp "$ROOT/packaging/flatpak/manifest.yml" "$STAGING/manifest.yml"
flatpak_repo="$STAGING/flatpak-repo"
flatpak-builder --force-clean --repo="$flatpak_repo" "$STAGING/flatpak-build" "$STAGING/manifest.yml"
flatpak build-bundle \
  --arch="$FLATPAK_ARCH" \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$flatpak_repo" "$OUTPUT/$NAME.flatpak" "$APP_ID" stable
