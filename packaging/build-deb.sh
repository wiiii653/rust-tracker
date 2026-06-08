#!/bin/bash
# Build a .deb package for rust-tracker
set -e

APP="rust-tracker"
VERSION="0.1.0"
ARCH="amd64"
DEB_DIR="packaging/deb"
BUILD_DIR="$DEB_DIR/${APP}_${VERSION}_${ARCH}"

echo "=== Building $APP .deb package v$VERSION ==="

# Build release binary
cargo build --release

# Create package structure
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/DEBIAN"
mkdir -p "$BUILD_DIR/usr/bin"
mkdir -p "$BUILD_DIR/usr/share/applications"
mkdir -p "$BUILD_DIR/usr/share/icons/hicolor/256x256/apps"
mkdir -p "$BUILD_DIR/usr/share/mime/packages"
mkdir -p "$BUILD_DIR/usr/share/doc/$APP"

# Copy binary
cp target/release/$APP "$BUILD_DIR/usr/bin/"

# Copy desktop file
cp resources/rust-tracker.desktop "$BUILD_DIR/usr/share/applications/"

# Copy MIME XML
cp resources/rust-tracker.xml "$BUILD_DIR/usr/share/mime/packages/"

# Create control file
cat > "$BUILD_DIR/DEBIAN/control" << EOF
Package: $APP
Version: $VERSION
Section: sound
Priority: optional
Architecture: $ARCH
Depends: libasound2 (>= 1.0), libudev1 (>= 200)
Maintainer: rust-tracker developers
Description: A modern Fast Tracker 2 clone for Linux
 rust-tracker is a music tracker application compatible with
 Fast Tracker 2 XM modules, as well as MOD, S3M, and IT formats.
 It features a pattern editor, sample editor, instrument editor,
 envelope graphs, audio visualization, MIDI input, and real-time
 playback via ALSA.
EOF

# Set permissions
chmod 755 "$BUILD_DIR/DEBIAN"
chmod 644 "$BUILD_DIR/DEBIAN/control"

# Build the package
dpkg-deb --build "$BUILD_DIR"

echo "=== Package built: $DEB_DIR/${APP}_${VERSION}_${ARCH}.deb ==="
