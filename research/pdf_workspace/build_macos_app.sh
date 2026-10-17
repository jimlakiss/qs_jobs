#!/bin/sh
set -eu
cd "$(dirname "$0")"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Library/Developer/CommandLineTools}"
cargo build --release --locked
app="target/iQs Drawing Viewer.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources/pdfium"
xcrun swiftc -O -module-cache-path target/swift-module-cache native/ocr.swift -o target/iqs-ocr
cp target/iqs-ocr target/release/iqs-ocr
cp target/iqs-ocr "$app/Contents/MacOS/iqs-ocr"
cp target/release/iqs-pdf-spike "$app/Contents/MacOS/iQsDrawingViewer"
cp runtime/lib/libpdfium.dylib "$app/Contents/Frameworks/"
cp runtime/LICENSE "$app/Contents/Resources/pdfium/"
cp -R runtime/licenses "$app/Contents/Resources/pdfium/"
# Build a native macOS icon from the versioned logo source.
iconset="target/AppIcon.iconset"
mkdir -p "$iconset"
for points in 16 32 128 256 512; do
  sips -z "$points" "$points" native/app-icon.png --out "$iconset/icon_${points}x${points}.png" >/dev/null
  pixels=$((points * 2))
  sips -z "$pixels" "$pixels" native/app-icon.png --out "$iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>iQsDrawingViewer</string>
<key>CFBundleIdentifier</key><string>local.iqs.drawingviewer.research</string>
<key>CFBundleName</key><string>iQs Drawing Viewer</string>
<key>CFBundleDisplayName</key><string>iQs Drawing Viewer</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>12.0</string>
</dict></plist>
PLIST
# Ad-hoc local signature; this is a development build, not a notarized release.
xattr -cr "$app"
# Finder/File Provider can reattach FinderInfo after recursive clearing.
xattr -d com.apple.FinderInfo "$app" 2>/dev/null || true
codesign --force --deep --sign - "$app"
codesign --verify --deep "$app"
printf 'Built: %s/%s\n' "$PWD" "$app"
