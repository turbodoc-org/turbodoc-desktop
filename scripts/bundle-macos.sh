#!/usr/bin/env bash
# Builds Turbodoc.app: an optimised binary, the app icon, the turbodoc:// link scheme and a
# signature (ad hoc unless TURBODOC_SIGN names a "Developer ID Application: …" identity).
#
#   scripts/bundle-macos.sh            # → dist/Turbodoc.app
#   scripts/bundle-macos.sh --install  # also copies it to ~/Applications and registers the scheme
set -euo pipefail
cd "$(dirname "$0")/.."

NAME=Turbodoc
ID=ai.turbodoc.desktop
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP="dist/$NAME.app"

cargo build --profile dist
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/dist/turbodoc "$APP/Contents/MacOS/turbodoc"

# The icon: every size iconutil wants, from the 1024 px master.
ICONSET=$(mktemp -d)/Turbodoc.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Turbodoc.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$ID</string>
  <key>CFBundleExecutable</key><string>turbodoc</string>
  <key>CFBundleIconFile</key><string>Turbodoc</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSHumanReadableCopyright</key><string>Turbodoc</string>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>$ID</string>
      <key>CFBundleURLSchemes</key><array><string>turbodoc</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

codesign --force --deep --options runtime --sign "${TURBODOC_SIGN:--}" "$APP" >/dev/null
echo "Built $APP ($VERSION)"

if [[ "${1:-}" == "--install" ]]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/$NAME.app"
  cp -R "$APP" "$HOME/Applications/"
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$HOME/Applications/$NAME.app"
  echo "Installed ~/Applications/$NAME.app"
fi
