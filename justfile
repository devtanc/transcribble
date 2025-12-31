# Transcribble justfile

# Default recipe - show available commands
default:
    @just --list

# Build the Tauri app (release mode with bundle)
build:
    cargo tauri build

# Build without bundling (faster, for testing)
build-fast:
    cargo tauri build --no-bundle

# Run in development mode
dev:
    cargo tauri dev

# Build and install to /Applications (signed with Developer ID certificate)
install: build
    @echo "Installing Transcribble to /Applications..."
    rm -rf "/Applications/Transcribble.app"
    cp -r "target/release/bundle/macos/Transcribble.app" "/Applications/"
    @echo "Installed successfully!"
    @echo "Code signature:"
    @codesign -dv "/Applications/Transcribble.app" 2>&1 | grep -E "Identifier|Authority"

# Reset macOS permissions for Transcribble (use if permissions are stuck/broken)
reset-permissions:
    @echo "Resetting Transcribble permissions..."
    tccutil reset Accessibility app.transcribble || true
    tccutil reset Microphone app.transcribble || true
    @echo "Done! Re-open the app and grant permissions again."

# Create DMG manually (workaround for Tauri bundler issues)
dmg: build
    #!/usr/bin/env bash
    set -euo pipefail
    DMG_NAME="Transcribble_$(grep '"version"' crates/transcribble-tauri/tauri.conf.json | head -1 | sed 's/.*: "//;s/".*//')_$(uname -m).dmg"
    DMG_PATH="target/release/bundle/dmg/${DMG_NAME}"
    APP_PATH="target/release/bundle/macos/Transcribble.app"
    mkdir -p target/release/bundle/dmg
    rm -f "${DMG_PATH}"
    echo "Creating ${DMG_NAME}..."
    hdiutil create -volname "Transcribble" -srcfolder "${APP_PATH}" -ov -format UDZO "${DMG_PATH}"
    codesign -s "Developer ID Application: Tanner Christensen (MLDWDJSY4Z)" "${DMG_PATH}"
    echo "Created: ${DMG_PATH}"

# Uninstall from /Applications
uninstall:
    @echo "Removing Transcribble from /Applications..."
    rm -rf "/Applications/Transcribble.app"
    @echo "Uninstalled successfully!"

# Build the CLI only
build-cli:
    cargo build --release --bin transcribble

# Install CLI to /usr/local/bin
install-cli: build-cli
    cp target/release/transcribble /usr/local/bin/
    @echo "CLI installed to /usr/local/bin/transcribble"

# Run the CLI
cli *ARGS:
    cargo run --bin transcribble -- {{ARGS}}

# Build frontend only
build-ui:
    cd ui && npm run build

# Run frontend dev server
dev-ui:
    cd ui && npm run dev

# Clean all build artifacts
clean:
    cargo clean
    rm -rf ui/dist

# Run cargo check
check:
    cargo check --workspace

# Run clippy
lint:
    cargo clippy --workspace

# Format code
fmt:
    cargo fmt --all

# Generate app icons from SVG source (requires librsvg: brew install librsvg)
icons:
    #!/usr/bin/env bash
    set -euo pipefail
    cd crates/transcribble-tauri/icons

    echo "Converting app-icon.svg to PNG formats with transparency..."

    # Use rsvg-convert for proper transparency support
    rsvg-convert -w 512 -h 512 app-icon.svg -o app-source.png

    # Create required PNG sizes
    sips -z 32 32 app-source.png --out 32x32.png
    sips -z 128 128 app-source.png --out 128x128.png
    sips -z 256 256 app-source.png --out 128x128@2x.png
    sips -z 512 512 app-source.png --out icon.png

    # Create iconset for macOS .icns
    echo "Creating macOS .icns..."
    mkdir -p icon.iconset
    sips -z 16 16 app-source.png --out icon.iconset/icon_16x16.png
    sips -z 32 32 app-source.png --out icon.iconset/icon_16x16@2x.png
    sips -z 32 32 app-source.png --out icon.iconset/icon_32x32.png
    sips -z 64 64 app-source.png --out icon.iconset/icon_32x32@2x.png
    sips -z 128 128 app-source.png --out icon.iconset/icon_128x128.png
    sips -z 256 256 app-source.png --out icon.iconset/icon_128x128@2x.png
    sips -z 256 256 app-source.png --out icon.iconset/icon_256x256.png
    sips -z 512 512 app-source.png --out icon.iconset/icon_256x256@2x.png
    sips -z 512 512 app-source.png --out icon.iconset/icon_512x512.png
    cp app-source.png icon.iconset/icon_512x512@2x.png
    iconutil -c icns icon.iconset

    # Cleanup
    rm -rf icon.iconset app-source.png

    echo "Icons generated successfully!"
    ls -la *.png *.icns
