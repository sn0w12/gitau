#!/usr/bin/env sh
set -eu

# Installs gitau from GitHub releases on Linux and macOS. Windows gets the
# NSIS installer link, since that one can't run from a shell.
#
#   curl -fsSL https://raw.githubusercontent.com/sn0w12/gitau/main/install.sh | sh
#
# Environment:
#   GITAU_VERSION      tag to install, "latest" (default) or e.g. "v0.2.0"
#   GITAU_INSTALL_DIR  Linux install target, ~/.local/opt/gitau by default
#   GITAU_APP_DIR      macOS install target, /Applications by default

REPO="https://github.com/sn0w12/gitau"
API="https://api.github.com/repos/sn0w12/gitau"
APP_NAME="gitau"

# Must match plugins.updater.pubkey in crates/tauri/tauri.conf.json.
UPDATER_PUBKEY="untrusted comment: minisign public key: 60D7189C516451B0
RWSwUWRRnBjXYO+H2l4HMFoEaeYYoU4CXwf9ruOUAO9mhho8uCVSKEsq
"

main() {
    platform="$(uname -s)"
    arch="$(uname -m)"
    version="${GITAU_VERSION:-latest}"

    if [ -n "${TMPDIR:-}" ] && [ -d "${TMPDIR}" ]; then
        temp="$(mktemp -d "$TMPDIR/gitau-XXXXXX")"
    else
        temp="$(mktemp -d "/tmp/gitau-XXXXXX")"
    fi
    trap 'rm -rf "$temp"' EXIT INT TERM

    if command -v curl >/dev/null 2>&1; then
        downloader="curl"
    elif command -v wget >/dev/null 2>&1; then
        downloader="wget"
    else
        echo "Could not find 'curl' or 'wget' in your path"
        exit 1
    fi

    mkdir -p "$HOME/.local/bin"

    case "$platform" in
        Linux) linux "$arch" "$version" ;;
        Darwin) macos "$arch" "$version" ;;
        MINGW* | MSYS* | CYGWIN*) windows "$version" ;;
        *)
            echo "Unsupported platform $platform"
            exit 1
            ;;
    esac

    case ":$PATH:" in
        *":$HOME/.local/bin:"*) ;;
        *)
            echo "Add ~/.local/bin to your PATH to run gitau from a terminal:"
            case "${SHELL:-}" in
                *zsh)
                    echo "   echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.zshrc"
                    echo "   source ~/.zshrc"
                    ;;
                *fish)
                    echo "   fish_add_path -U \$HOME/.local/bin"
                    ;;
                *)
                    echo "   echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.bashrc"
                    echo "   source ~/.bashrc"
                    ;;
            esac
            ;;
    esac
}

latest_tag() {
    tag="$(fetch_text "$API/releases/latest" |
        sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
        head -n 1)"
    if [ -z "$tag" ]; then
        echo "Could not read the latest release tag from $API/releases/latest."
        echo "Pass GITAU_VERSION=<tag> to install a specific one."
        exit 1
    fi
    echo "$tag"
}

fetch_text() {
    if [ "$downloader" = "curl" ]; then
        curl -fsSL "$1"
    else
        wget -q -O- "$1"
    fi
}

fetch() {
    if [ "$downloader" = "curl" ]; then
        curl -fL --progress-bar "$1" -o "$2"
    else
        wget -q -O "$2" "$1"
    fi
}

# Downloads the first URL that resolves, so a release matrix that ships both
# per-arch and universal bundles still installs. Echoes the URL that worked.
download_first() {
    target="$1"
    shift
    for url in "$@"; do
        if fetch "$url" "$target"; then
            echo "$url"
            return 0
        fi
    done
    return 1
}

# Tauri's updater signs releases with minisign, and wraps the signature in one
# more layer of base64. Verify against the same public key the app's updater
# plugin uses, so the install path checks what the update path checks.
verify_bundle() {
    bundle="$1"
    url="$2"

    if ! command -v minisign >/dev/null 2>&1; then
        echo "Warning: minisign not found, skipping signature verification."
        echo "Install it, or download the AppImage by hand from $REPO/releases"
        return 0
    fi

    sig="$temp/signature"
    if ! fetch "$url.sig" "$sig"; then
        echo "Warning: no signature published for $(basename "$url"), skipping verification."
        return 0
    fi

    # BSD base64 spells it -D, GNU spells it -d.
    if ! base64 -d "$sig" > "$temp/signature.minisig" 2>/dev/null; then
        base64 -D "$sig" > "$temp/signature.minisig"
    fi
    printf '%s' "$UPDATER_PUBKEY" > "$temp/gitau.pub"

    if minisign -V -p "$temp/gitau.pub" -x "$temp/signature.minisig" -m "$bundle" >/dev/null 2>&1; then
        echo "Signature verified."
    else
        echo "Signature check failed for $(basename "$url"). Refusing to install."
        exit 1
    fi
}

linux() {
    arch="$1"
    version="$2"

    case "$arch" in
        x86_64 | amd64) rust_arch="amd64" ;;
        aarch64 | arm64) rust_arch="aarch64" ;;
        *)
            echo "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    tag="$(resolve_tag "$version")"
    echo "Installing $APP_NAME $tag for linux-$rust_arch"

    bundle="$temp/$APP_NAME.AppImage"
    if ! bundle_url="$(download_first "$bundle" \
        "$REPO/releases/download/$tag/${APP_NAME}_${tag#v}_${rust_arch}.AppImage" \
        "$REPO/releases/download/latest/${APP_NAME}_${rust_arch}.AppImage")"; then
        echo "No AppImage published for linux-$rust_arch in $tag."
        echo "Grab a build by hand from $REPO/releases/tag/$tag"
        exit 1
    fi
    verify_bundle "$bundle" "$bundle_url"
    chmod +x "$bundle"

    install_dir="${GITAU_INSTALL_DIR:-$HOME/.local/opt/$APP_NAME}"
    mkdir -p "$install_dir"
    mv "$bundle" "$install_dir/$APP_NAME.AppImage"

    data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
    ln -sf "$install_dir/$APP_NAME.AppImage" "$HOME/.local/bin/$APP_NAME"
    install_desktop_entry "$install_dir/$APP_NAME.AppImage" "$data_home" "$temp"

    echo "$APP_NAME $tag installed. Run with '$APP_NAME'"
}

install_desktop_entry() {
    appimage="$1"
    data_home="$2"
    temp="$3"

    desktop_dir="$data_home/applications"

    # The AppImage runtime unpacks a single path per call, which is much
    # cheaper than unpacking the whole 80MB image to read these two files.
    (cd "$temp" &&
        "$appimage" --appimage-extract 'usr/share/applications/*.desktop' >/dev/null 2>&1 &&
        "$appimage" --appimage-extract 'usr/share/icons/hicolor/*/apps/*.png' >/dev/null 2>&1) ||
        return 0

    desktop_src="$(ls -1 "$temp/squashfs-root/usr/share/applications/"*.desktop 2>/dev/null | head -n 1)"
    if [ -z "$desktop_src" ]; then
        return 0
    fi

    icon_src=""
    icon_size=""
    for size in 256x256@2 256x256 128x128 64x64 32x32 16x16; do
        if [ -f "$temp/squashfs-root/usr/share/icons/hicolor/$size/apps/$APP_NAME.png" ]; then
            icon_src="$temp/squashfs-root/usr/share/icons/hicolor/$size/apps/$APP_NAME.png"
            icon_size="$size"
            break
        fi
    done

    mkdir -p "$desktop_dir"
    if [ -n "$icon_src" ]; then
        mkdir -p "$data_home/icons/hicolor/$icon_size/apps"
        cp "$icon_src" "$data_home/icons/hicolor/$icon_size/apps/$APP_NAME.png"
    fi

    desktop_file="$desktop_dir/$APP_NAME.desktop"
    cp "$desktop_src" "$desktop_file"
    # Absolute paths keep the entry working without a refreshed icon cache and
    # without the AppImage sitting on PATH.
    sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/$APP_NAME|" "$desktop_file"
    sed -i "s|^Icon=.*|Icon=$data_home/icons/hicolor/$icon_size/apps/$APP_NAME.png|" "$desktop_file"
    sed -i "s|^Categories=.*|Categories=Development;|" "$desktop_file"
}

macos() {
    arch="$1"
    version="$2"

    case "$arch" in
        arm64 | aarch64) target_arch="aarch64" ;;
        x86_64 | amd64) target_arch="x64" ;;
        *)
            echo "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    tag="$(resolve_tag "$version")"
    echo "Installing $APP_NAME $tag for macOS"

    dmg="$temp/$APP_NAME.dmg"
    if ! dmg_url="$(download_first "$dmg" \
        "$REPO/releases/download/$tag/${APP_NAME}_${tag#v}_${target_arch}.dmg" \
        "$REPO/releases/download/$tag/${APP_NAME}_${tag#v}_universal.dmg" \
        "$REPO/releases/download/latest/${APP_NAME}_universal.dmg")"; then
        echo "No disk image published for macOS in $tag."
        echo "Grab a build by hand from $REPO/releases/tag/$tag"
        exit 1
    fi
    verify_bundle "$dmg" "$dmg_url"

    hdiutil attach -quiet "$dmg" -mountpoint "$temp/mount"
    app="$(cd "$temp/mount"; echo *.app)"

    app_dir="${GITAU_APP_DIR:-/Applications}"
    if [ ! -w "$app_dir" ]; then
        app_dir="$HOME/Applications"
    fi
    mkdir -p "$app_dir"

    echo "Installing $app"
    rm -rf "$app_dir/$app"
    ditto "$temp/mount/$app" "$app_dir/$app"
    hdiutil detach -quiet "$temp/mount"

    ln -sf "$app_dir/$app/Contents/MacOS/$APP_NAME" "$HOME/.local/bin/$APP_NAME"

    echo "$APP_NAME $tag installed. Run with 'open $app_dir/$app' or '$APP_NAME'"
}

windows() {
    version="$1"
    tag="$(resolve_tag "$version")"
    echo "gitau on Windows installs through its NSIS installer, which needs a shell of its own."
    echo "Download and run it from here:"
    echo "   $REPO/releases/download/$tag/${APP_NAME}_${tag#v}_x64-setup.exe"
    echo "Or open the releases page: $REPO/releases/latest"
}

resolve_tag() {
    if [ "$1" = "latest" ]; then
        latest_tag
    else
        case "$1" in
            v*) echo "$1" ;;
            *) echo "v$1" ;;
        esac
    fi
}

main "$@"