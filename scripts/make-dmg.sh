#!/usr/bin/env bash
# Builds a drag-and-drop .dmg (app + Applications shortcut) from a macOS app bundle.
# Usage: scripts/make-dmg.sh <path/to/Disk Analyzer.app> [output.dmg]
set -euo pipefail

APP="${1:?usage: $0 <app bundle> [output.dmg]}"
OUT="${2:-target/Disk Analyzer.dmg}"
VOLNAME="Disk Analyzer"

STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"

rm -f "$OUT"
hdiutil create -volname "$VOLNAME" -srcfolder "$STAGING" -ov -format UDZO "$OUT" >/dev/null
echo "Created $OUT"
