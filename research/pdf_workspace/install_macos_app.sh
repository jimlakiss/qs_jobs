#!/bin/sh
# Install the built local app at a stable location; leave session data untouched.
set -eu
cd "$(dirname "$0")"
app="target/iQs Drawing Viewer.app"
destination="${IQS_INSTALL_DIR:-/Applications}/iQs Drawing Viewer.app"
[ -d "$app" ] || { printf 'Build the app with ./build_macos_app.sh first.\n' >&2; exit 1; }
codesign --verify --deep "$app"
if [ -e "$destination" ]; then
  identifier=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$destination/Contents/Info.plist")
  [ "$identifier" = "local.iqs.drawingviewer.research" ] || {
    printf 'Refusing to replace an unrelated application: %s\n' "$destination" >&2
    exit 1
  }
fi
# A normal Quit flushes the latest viewer session before updating the bundle.
if pgrep -f '/Contents/MacOS/iQsDrawingViewer$' >/dev/null; then
  osascript -e 'tell application id "local.iqs.drawingviewer.research" to quit'
  attempt=0
  while pgrep -f '/Contents/MacOS/iQsDrawingViewer$' >/dev/null; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 30 ] || { printf 'Viewer is still running; installation cancelled.\n' >&2; exit 1; }
    sleep 0.2
  done
fi
mkdir -p "$(dirname "$destination")"
ditto --norsrc --noextattr "$app" "$destination"
xattr -d com.apple.FinderInfo "$destination" 2>/dev/null || true
codesign --verify --deep "$destination"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$destination"
printf 'Installed: %s\n' "$destination"
