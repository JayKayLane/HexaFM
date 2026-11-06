#!/bin/sh
# Builds every Hexa FM download into dist/:
#   HexaFM-0.1.0.pkg             Mac installer (choose app / AU / VST3 / CLAP, terms + GPL3)
#   HexaFM-0.1.0.dmg             Mac disk image (installer, drag-install app, plugins, manual, licence)
#   HexaFM-Setup-0.1.0.exe       Windows installer (app / VST3 / CLAP, terms + GPL3, uninstaller)
#   HexaFM-Mac-Plugins.zip       AU, VST3 and CLAP files to drop into plugin folders
#   HexaFM-Windows-Plugins.zip   VST3 and CLAP files to drop into plugin folders
#   HexaFM-Windows-App.zip       the Windows app on its own (no installer)
#   HexaFM-Source-0.1.0.zip      source code (GPL v3)
#   Hexa FM Manual.pdf
# Run from app/:  sh installer/make_release.sh
# Needs: Rust targets aarch64/x86_64-apple-darwin and x86_64-pc-windows-msvc, the xwin libraries
# (see installer/win-env.sh), CMake (PATH or /tmp/avdx/tools/bin) and Google Chrome for the PDF.
set -e
cd "$(dirname "$0")/.."
. "$HOME/.cargo/env" 2>/dev/null || true
export PATH="/tmp/avdx/tools/bin:$PATH"
V=0.1.0
STAMP=$(date +%Y%m%d%H%M%S)
S="target/release-stage-$STAMP"
mkdir -p "$S" dist
echo "== manual"
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --no-pdf-header-footer \
  --generate-pdf-document-outline --print-to-pdf="$PWD/manual/Hexa FM Manual.pdf" "file://$PWD/manual/manual.html" >/dev/null 2>&1
cp "manual/Hexa FM Manual.pdf" dist/

echo "== Mac standalone (universal)"
cargo build --release --target aarch64-apple-darwin --bin hexa-fm
cargo build --release --target x86_64-apple-darwin --bin hexa-fm
APP="$S/app/Hexa FM.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create target/aarch64-apple-darwin/release/hexa-fm target/x86_64-apple-darwin/release/hexa-fm -output "$APP/Contents/MacOS/Hexa FM"
cp installer/icon/HexaFM.icns "$APP/Contents/Resources/HexaFM.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>Hexa FM</string>
  <key>CFBundleIdentifier</key><string>app.hexafm.synth</string>
  <key>CFBundleName</key><string>Hexa FM</string>
  <key>CFBundleDisplayName</key><string>Hexa FM</string>
  <key>CFBundleIconFile</key><string>HexaFM</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$V</string>
  <key>CFBundleVersion</key><string>$V</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Hexa FM does not record audio; macOS may ask because it opens an audio device.</string>
  <key>CFBundleDocumentTypes</key><array>
    <dict><key>CFBundleTypeName</key><string>SysEx bank</string><key>CFBundleTypeRole</key><string>Viewer</string><key>CFBundleTypeExtensions</key><array><string>syx</string></array></dict>
    <dict><key>CFBundleTypeName</key><string>Hexa FM sound</string><key>CFBundleTypeRole</key><string>Editor</string><key>CFBundleTypeExtensions</key><array><string>hexafm</string></array></dict>
  </array>
</dict></plist>
PLIST

echo "== Mac plugins (universal VST3 + CLAP, AU wrapper)"
cargo xtask bundle-universal hexa-fm-plugin --release
cmake -S au-wrapper -B target/au-build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES="arm64;x86_64" \
  -DHEXA_CLAP="$PWD/target/bundled/Hexa FM.clap" >/dev/null
cmake --build target/au-build
# The wrapper writes the AU's Info.plist (with its AudioComponents entry) only when it re-links;
# a reconfigure can leave a plain plist behind, which makes the AU invisible to hosts. Always copy it.
cp target/au-build/HexaFM_auv2-build-helper-output/auv2_Info.plist "target/au-build/Hexa FM.component/Contents/Info.plist"
plutil -extract AudioComponents raw "target/au-build/Hexa FM.component/Contents/Info.plist" >/dev/null || { echo "AU Info.plist has no AudioComponents"; exit 1; }
mkdir -p "$S/vst3" "$S/clap" "$S/au"
cp -R "target/bundled/Hexa FM.vst3" "$S/vst3/"
cp -R "target/bundled/Hexa FM.clap" "$S/clap/"
cp -R "target/au-build/Hexa FM.component" "$S/au/"

echo "== signing (ad-hoc: runs on any Mac after the one-time Open Anyway step; see manual chapter 3)"
for b in "$APP" "$S/vst3/Hexa FM.vst3" "$S/clap/Hexa FM.clap" "$S/au/Hexa FM.component"; do
  xattr -cr "$b" 2>/dev/null || true
  codesign --force --deep --sign - --timestamp=none "$b"
done

echo "== Mac installer (.pkg)"
P="$S/pkgs"
mkdir -p "$P" "$S/scripts-au"
cat > "$S/scripts-au/postinstall" <<'SH'
#!/bin/sh
# make Logic/GarageBand notice the new Audio Unit
killall -9 AudioComponentRegistrar 2>/dev/null || true
exit 0
SH
chmod +x "$S/scripts-au/postinstall"
for c in app vst3 clap au; do
  pkgbuild --analyze --root "$S/$c" "$S/$c.plist" >/dev/null
  plutil -replace 0.BundleIsRelocatable -bool NO "$S/$c.plist"
done
pkgbuild --root "$S/app" --component-plist "$S/app.plist" --identifier app.hexafm.synth.app --version $V --install-location /Applications "$P/app.pkg"
pkgbuild --root "$S/vst3" --component-plist "$S/vst3.plist" --identifier app.hexafm.synth.vst3 --version $V --install-location /Library/Audio/Plug-Ins/VST3 "$P/vst3.pkg"
pkgbuild --root "$S/clap" --component-plist "$S/clap.plist" --identifier app.hexafm.synth.clap --version $V --install-location /Library/Audio/Plug-Ins/CLAP "$P/clap.pkg"
pkgbuild --root "$S/au" --component-plist "$S/au.plist" --scripts "$S/scripts-au" --identifier app.hexafm.synth.au --version $V --install-location /Library/Audio/Plug-Ins/Components "$P/au.pkg"
mkdir -p "$S/res"
cp installer/resources/welcome.html installer/resources/conclusion.html installer/TERMS.txt "$S/res/"
productbuild --distribution installer/distribution.xml --resources "$S/res" --package-path "$P" "dist/HexaFM-$V.pkg"

echo "== Mac disk image (.dmg)"
D="$S/dmg"
mkdir -p "$D/Plugins"
cp "dist/HexaFM-$V.pkg" "$D/Install Hexa FM.pkg"
cp -R "$APP" "$D/"
ln -s /Applications "$D/Applications"
cp -R "$S/au/Hexa FM.component" "$S/vst3/Hexa FM.vst3" "$S/clap/Hexa FM.clap" "$D/Plugins/"
cp "manual/Hexa FM Manual.pdf" "$D/"
cp installer/TERMS.txt "$D/Terms and licence.txt"
cp COPYING "$D/GPL-3.0 licence.txt"
cat > "$D/Read me first.txt" <<'TXT'
HEXA FM 0.1.0

Easiest: double-click "Install Hexa FM.pkg" and click Customize to choose the app and the AU, VST3 and CLAP plugins.
Or drag "Hexa FM.app" onto Applications, and copy the files in "Plugins" into your plugin folders (manual, chapter 5).

If macOS says it can't check or open a file: System Settings > Privacy & Security > Open Anyway.
The manual (Hexa FM Manual.pdf, chapter 3) explains every macOS security message step by step.
TXT
hdiutil create -volname "Hexa FM $V" -srcfolder "$D" -ov -format UDZO "dist/HexaFM-$V.dmg" >/dev/null

echo "== Mac drop-in plugins (.zip)"
mkdir -p "$S/zip-mac/Hexa FM Plugins (Mac)"
cp -R "$S/au/Hexa FM.component" "$S/vst3/Hexa FM.vst3" "$S/clap/Hexa FM.clap" "$S/zip-mac/Hexa FM Plugins (Mac)/"
cp "$D/Read me first.txt" "$S/zip-mac/Hexa FM Plugins (Mac)/"
ditto -c -k --keepParent "$S/zip-mac/Hexa FM Plugins (Mac)" dist/HexaFM-Mac-Plugins.zip

echo "== Windows app, plugins and installer"
. installer/win-env.sh
cargo build --release --target x86_64-pc-windows-msvc --bin hexa-fm
cargo build --release --target x86_64-pc-windows-msvc -p hexa-fm-plugin
cargo build --release --target x86_64-pc-windows-msvc -p hexa-fm-setup
cp target/x86_64-pc-windows-msvc/release/HexaFM-Setup.exe "dist/HexaFM-Setup-$V.exe"
W="$S/zip-win/Hexa FM Plugins (Windows)"
mkdir -p "$W/Hexa FM.vst3/Contents/x86_64-win"
cp target/x86_64-pc-windows-msvc/release/hexa_fm_plugin.dll "$W/Hexa FM.vst3/Contents/x86_64-win/Hexa FM.vst3"
cp target/x86_64-pc-windows-msvc/release/hexa_fm_plugin.dll "$W/Hexa FM.clap"
cat > "$W/Read me first.txt" <<'TXT'
HEXA FM 0.1.0 - plugins for Windows (64-bit)

Copy "Hexa FM.vst3" (the whole folder) into  C:\Program Files\Common Files\VST3
Copy "Hexa FM.clap"                     into  C:\Program Files\Common Files\CLAP
Then rescan plugins in your music software. Or simply run HexaFM-Setup-0.1.0.exe.
See the manual (Hexa FM Manual.pdf), chapters 4 and 5.
TXT
ditto -c -k --keepParent "$W" dist/HexaFM-Windows-Plugins.zip
A="$S/zip-winapp/Hexa FM (Windows)"
mkdir -p "$A"
cp target/x86_64-pc-windows-msvc/release/hexa-fm.exe "$A/Hexa FM.exe"
cp "manual/Hexa FM Manual.pdf" "$A/"
cp installer/TERMS.txt "$A/Terms and licence.txt"
ditto -c -k --keepParent "$A" dist/HexaFM-Windows-App.zip

echo "== source code"
SRC="$S/src/hexa-fm-$V"
mkdir -p "$SRC"
for f in Cargo.toml Cargo.lock bundler.toml COPYING MECHANICS.md mechanics.json src assets plugin xtask win-installer au-wrapper installer manual .cargo; do
  [ -e "$f" ] && cp -R "$f" "$SRC/"
done
ditto -c -k --keepParent "$SRC" "dist/HexaFM-Source-$V.zip"

echo "== done"
ls -la dist
