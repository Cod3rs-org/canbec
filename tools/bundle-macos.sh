#!/bin/sh
# Wrap the release binary into canbec.app (+ zip) with the sprout icon: tools/bundle-macos.sh
# ponytail: unsigned; Gatekeeper wants right-click → Open the first time. Signing + notarizing
# needs an Apple Developer ID.
set -e
version=$(sed -n 's/^version = "\(.*\)"/\1/p' src/app/Cargo.toml | head -1)
app=dist/canbec.app
rm -rf dist && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/canbec "$app/Contents/MacOS/canbec"

set=dist/AppIcon.iconset && mkdir -p $set
src=src/infrastructure/src/icons/sprout.png
for s in 16 32 128 256; do sips -z $s $s $src --out $set/icon_${s}x${s}.png >/dev/null; done
for s in 16 128; do d=$((s * 2)); sips -z $d $d $src --out $set/icon_${s}x${s}@2x.png >/dev/null; done
iconutil -c icns $set -o "$app/Contents/Resources/AppIcon.icns" && rm -rf $set

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>canbec</string>
  <key>CFBundleDisplayName</key><string>canbec</string>
  <key>CFBundleIdentifier</key><string>org.cod3rs.canbec</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleExecutable</key><string>canbec</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSCameraUsageDescription</key><string>A website you opened asks to use the camera.</string>
  <key>NSMicrophoneUsageDescription</key><string>A website you opened asks to use the microphone.</string>
</dict></plist>
PLIST
ditto -c -k --keepParent "$app" "dist/canbec-$version-macos-$(uname -m).zip"
echo "built $app and dist/canbec-$version-macos-$(uname -m).zip"
