#!/usr/bin/env bash
# Builds a double-clickable macOS app bundle (no Terminal window) around a compiled binary.
# Usage: scripts/bundle-macos.sh <path/to/rs-disk-analyzer> [output.app]
set -euo pipefail

BIN="${1:?usage: $0 <binary> [output.app]}"
APP="${2:-target/Disk Analyzer.app}"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/rs-disk-analyzer"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Disk Analyzer</string>
  <key>CFBundleDisplayName</key><string>Disk Analyzer</string>
  <key>CFBundleIdentifier</key><string>io.github.laurenthouel.rs-disk-analyzer</string>
  <key>CFBundleExecutable</key><string>rs-disk-analyzer</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
</dict>
</plist>
PLIST

# Ad-hoc signature: required for arm64 binaries to run once the bundle is modified.
codesign --force --deep --sign - "$APP"
echo "Created $APP"
