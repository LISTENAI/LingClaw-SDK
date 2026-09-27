#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile=debug
case "${1:-}" in
  "") ;;
  --release) profile=release ;;
  *) printf '%s\n' 'usage: package-macos.sh [--release]' >&2; exit 2 ;;
esac
cargo build --locked "$@"
app="target/LingClaw Simulator.app"
mkdir -p "$app/Contents/MacOS"
cp "target/$profile/lingclaw-sdk" "$app/Contents/MacOS/lingclaw-sdk"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>LingClaw Simulator</string>
<key>CFBundleDisplayName</key><string>LingClaw Simulator</string>
<key>CFBundleIdentifier</key><string>com.listenai.lingclaw.simulator</string>
<key>CFBundleExecutable</key><string>lingclaw-sdk</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
printf '%s\n' "$app"
