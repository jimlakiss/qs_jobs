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
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>iQsDrawingViewer</string>
<key>CFBundleIdentifier</key><string>local.iqs.drawingviewer.research</string>
<key>CFBundleName</key><string>iQs Drawing Viewer</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>12.0</string>
</dict></plist>
PLIST
# Ad-hoc local signature; this is a development build, not a notarized release.
xattr -cr "$app"
codesign --force --deep --sign - "$app"
printf 'Built: %s/%s\n' "$PWD" "$app"
