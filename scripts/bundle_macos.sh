#!/usr/bin/env bash
# ==============================================================================
# VideoCropTrim - macOS .app & .dmg Packaging Script
# ==============================================================================
# Usage:
#   chmod +x scripts/bundle_macos.sh
#   ./scripts/bundle_macos.sh
#
# Options:
#   --universal    Build universal binary (arm64 + x86_64)
# ==============================================================================

set -euo pipefail

APP_NAME="VideoCropTrim"
BUNDLE_ID="com.videocroptrim.app"
VERSION="0.1.2"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/target/mac_bundle"
APP_BUNDLE="${OUTPUT_DIR}/${APP_NAME}.app"
DMG_NAME="${APP_NAME}-${VERSION}-macOS.dmg"
DMG_PATH="${OUTPUT_DIR}/${DMG_NAME}"

BUILD_UNIVERSAL=false
if [[ "${1:-}" == "--universal" ]]; then
    BUILD_UNIVERSAL=true
fi

echo "=================================================="
echo " Packaging ${APP_NAME} for macOS"
echo "=================================================="

cd "${ROOT_DIR}"

# 1. Compile release binary
if [ "$BUILD_UNIVERSAL" = true ]; then
    echo "🔨 Building Universal Binary (aarch64 + x86_64)..."
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    
    mkdir -p "${ROOT_DIR}/target/universal-release"
    lipo -create -output "${ROOT_DIR}/target/universal-release/video_crop_trim" \
        "${ROOT_DIR}/target/aarch64-apple-darwin/release/video_crop_trim" \
        "${ROOT_DIR}/target/x86_64-apple-darwin/release/video_crop_trim"
    BIN_SRC="${ROOT_DIR}/target/universal-release/video_crop_trim"
else
    echo "🔨 Building Native Release Binary..."
    cargo build --release
    BIN_SRC="${ROOT_DIR}/target/release/video_crop_trim"
fi

# 2. Setup .app directory structure
echo "📦 Creating .app bundle structure..."
rm -rf "${OUTPUT_DIR}"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

# Copy binary
cp "${BIN_SRC}" "${APP_BUNDLE}/Contents/MacOS/video_crop_trim"
chmod +x "${APP_BUNDLE}/Contents/MacOS/video_crop_trim"

# Copy Info.plist
if [ -f "${SCRIPT_DIR}/Info.plist" ]; then
    cp "${SCRIPT_DIR}/Info.plist" "${APP_BUNDLE}/Contents/Info.plist"
else
    cat <<EOF > "${APP_BUNDLE}/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>video_crop_trim</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
fi

# Copy icon if available
if [ -f "${SCRIPT_DIR}/AppIcon.icns" ]; then
    cp "${SCRIPT_DIR}/AppIcon.icns" "${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
fi

# Write PkgInfo
echo -n "APPL????" > "${APP_BUNDLE}/Contents/PkgInfo"

echo "✅ App bundle created at: ${APP_BUNDLE}"

# 3. Create .dmg installer image
echo "💿 Building .dmg installer..."
DMG_STAGE="${OUTPUT_DIR}/dmg_stage"
rm -rf "${DMG_STAGE}"
mkdir -p "${DMG_STAGE}"

# Copy .app to staging
cp -R "${APP_BUNDLE}" "${DMG_STAGE}/"

# Create Applications shortcut link
ln -s /Applications "${DMG_STAGE}/Applications"

# Generate DMG using macOS native hdiutil
rm -f "${DMG_PATH}"
hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${DMG_STAGE}" \
    -ov \
    -format UDZO \
    "${DMG_PATH}"

rm -rf "${DMG_STAGE}"

echo "=================================================="
echo "🎉 Build & Packaging Completed Successfully!"
echo "   - .app Bundle: ${APP_BUNDLE}"
echo "   - .dmg Installer: ${DMG_PATH}"
echo "=================================================="

