#!/bin/bash
# Build an AppImage for rust-tracker
set -e

APP="rust-tracker"
VERSION="0.1.0"
APPDIR="packaging/AppDir"

echo "=== Building $APP AppImage v$VERSION ==="

# Build release binary
cargo build --release

# Create AppDir structure
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
mkdir -p "$APPDIR/usr/share/metainfo"

# Copy binary
cp target/release/$APP "$APPDIR/usr/bin/"

# Copy desktop file
cp resources/rust-tracker.desktop "$APPDIR/usr/share/applications/"

# Create AppRun
cat > "$APPDIR/AppRun" << 'EOF'
#!/bin/bash
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
export LD_LIBRARY_PATH="$HERE/usr/lib:$LD_LIBRARY_PATH"
exec "$HERE/usr/bin/rust-tracker" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Copy icon (placeholder if none exists)
if [ ! -f "resources/rust-tracker.png" ]; then
    # Create a minimal PNG placeholder
    echo "Note: No icon found, creating placeholder..."
    # Use a 1x1 transparent PNG as minimal placeholder
    printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x01\x00\x00\x00\x01\x00\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82' > "$APPDIR/usr/share/icons/hicolor/256x256/apps/rust-tracker.png"
else
    cp resources/rust-tracker.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/"
fi
cp resources/rust-tracker.desktop "$APPDIR/"

# Check for appimagetool
if command -v appimagetool &> /dev/null; then
    appimagetool "$APPDIR" "packaging/${APP}-${VERSION}-x86_64.AppImage"
    echo "=== AppImage built: packaging/${APP}-${VERSION}-x86_64.AppImage ==="
else
    echo "=== AppDir prepared at $APPDIR ==="
    echo "Install appimagetool to build the AppImage:"
    echo "  wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage"
    echo "  chmod +x appimagetool-x86_64.AppImage"
    echo "  ./appimagetool-x86_64.AppImage $APPDIR packaging/${APP}-${VERSION}-x86_64.AppImage"
fi
