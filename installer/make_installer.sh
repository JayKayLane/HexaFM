#!/bin/sh
# Builds Hexa FM.app and a macOS installer package (dist/HexaFM-0.1.0.pkg).
# Run from the app/ folder:  sh installer/make_installer.sh
set -e
cd "$(dirname "$0")/.."
VERSION=0.1.0
cargo build --release
STAGE="stage-$(date +%Y%m%d%H%M%S)"
APP="$STAGE/Hexa FM.app"
mkdir -p dist
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/hexa-fm "$APP/Contents/MacOS/Hexa FM"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Hexa FM</string>
  <key>CFBundleIdentifier</key><string>app.hexafm.synth</string>
  <key>CFBundleName</key><string>Hexa FM</string>
  <key>CFBundleDisplayName</key><string>Hexa FM</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>SysEx bank</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleTypeExtensions</key><array><string>syx</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Hexa FM sound</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>CFBundleTypeExtensions</key><array><string>hexafm</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST
xattr -cr "$APP" 2>/dev/null || true
export COPYFILE_DISABLE=1
# ad-hoc signature so macOS will run it on this computer
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
pkgbuild --root "$STAGE" --identifier app.hexafm.synth.pkg --version "$VERSION" --install-location /Applications "dist/HexaFM-$VERSION.pkg"
cp -R "$APP" dist/
echo "App bundle: $(pwd)/dist/Hexa FM.app"
echo "Installer: $(pwd)/dist/HexaFM-$VERSION.pkg"
