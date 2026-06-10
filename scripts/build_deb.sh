#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PACKAGE="local-speak-input"
VERSION="$(python3 - <<'PY'
import tomllib
with open("pyproject.toml", "rb") as fh:
    print(tomllib.load(fh)["project"]["version"])
PY
)"
ARCH="${LOCAL_SPEAK_DEB_ARCH:-amd64}"
MODEL_REPO="${LOCAL_SPEAK_BUNDLE_MODEL_REPO:-Systran/faster-whisper-base}"
MODEL_NAME="${LOCAL_SPEAK_BUNDLE_MODEL_NAME:-faster-whisper-base}"
BUILD_DIR="$ROOT_DIR/build/deb"
ROOTFS="$BUILD_DIR/rootfs"
VENV_DIR="$ROOTFS/opt/local-speak-input/venv"
MODEL_DIR="$ROOTFS/opt/local-speak-input/models/$MODEL_NAME"
DIST_DIR="$ROOT_DIR/dist"
DEB_PATH="$DIST_DIR/${PACKAGE}_${VERSION}_${ARCH}.deb"

rm -rf "$BUILD_DIR"
mkdir -p "$ROOTFS/DEBIAN" "$DIST_DIR"
mkdir -p "$ROOTFS/opt/local-speak-input" "$ROOTFS/usr/bin"
mkdir -p "$ROOTFS/etc/local-speak-input"
mkdir -p "$ROOTFS/usr/share/applications"
mkdir -p "$ROOTFS/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$ROOTFS/usr/share/metainfo"
mkdir -p "$ROOTFS/usr/share/doc/$PACKAGE"
mkdir -p "$ROOTFS/usr/lib/systemd/user"

uv venv --relocatable --python python3 "$VENV_DIR"
uv pip install --python "$VENV_DIR/bin/python" --link-mode copy '.[whisper,mic]'

"$VENV_DIR/bin/python" - <<PY
from huggingface_hub import snapshot_download
snapshot_download(
    repo_id="$MODEL_REPO",
    local_dir="$MODEL_DIR",
    local_dir_use_symlinks=False,
)
PY

install -m 0755 packaging/scripts/local-speak "$ROOTFS/usr/bin/local-speak"
install -m 0755 packaging/scripts/local-speak-gui "$ROOTFS/usr/bin/local-speak-gui"
install -m 0755 packaging/scripts/local-speak-daemon "$ROOTFS/usr/bin/local-speak-daemon"
install -m 0755 packaging/scripts/local-speak-tray "$ROOTFS/usr/bin/local-speak-tray"
install -m 0644 packaging/debian/config.toml "$ROOTFS/etc/local-speak-input/config.toml"
install -m 0644 packaging/debian/local-speak-input.desktop "$ROOTFS/usr/share/applications/local-speak-input.desktop"
install -m 0644 packaging/icons/local-speak-input.svg "$ROOTFS/usr/share/icons/hicolor/scalable/apps/local-speak-input.svg"
install -m 0644 packaging/debian/local-speak-input.metainfo.xml "$ROOTFS/usr/share/metainfo/local-speak-input.metainfo.xml"
install -m 0644 packaging/systemd/local-speak-input.service "$ROOTFS/usr/lib/systemd/user/local-speak-input.service"
install -m 0755 packaging/debian/postinst "$ROOTFS/DEBIAN/postinst"
install -m 0755 packaging/debian/postrm "$ROOTFS/DEBIAN/postrm"
install -m 0644 README.md "$ROOTFS/usr/share/doc/$PACKAGE/README.md"
install -m 0644 docs/architecture.md "$ROOTFS/usr/share/doc/$PACKAGE/architecture.md"
install -m 0644 docs/linux-setup.md "$ROOTFS/usr/share/doc/$PACKAGE/linux-setup.md"
install -m 0644 docs/model-research.md "$ROOTFS/usr/share/doc/$PACKAGE/model-research.md"
install -m 0644 docs/packaging.md "$ROOTFS/usr/share/doc/$PACKAGE/packaging.md"
install -m 0644 docs/test-report.md "$ROOTFS/usr/share/doc/$PACKAGE/test-report.md"
install -m 0644 packaging/debian/copyright "$ROOTFS/usr/share/doc/$PACKAGE/copyright"
gzip -9n "$ROOTFS/usr/share/doc/$PACKAGE/"*.md

cat >"$ROOTFS/DEBIAN/control" <<EOF
Package: $PACKAGE
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: local-speak-input contributors
Depends: python3 (>= 3.11), python3-gi, gir1.2-gtk-3.0, gir1.2-appindicator3-0.1, gnome-shell-extension-appindicator, libportaudio2, libx11-6, libnotify-bin, ffmpeg, pipewire-bin, wireplumber, xdotool, xclip, xdg-utils, ca-certificates
Installed-Size: $(du -sk "$ROOTFS" | cut -f1)
Description: VoxFlow Input voice input method for Linux
 VoxFlow Input provides a local web console, desktop launcher, top-bar
 indicator, configurable background hotkey daemon, desktop notifications, and
 command-line tools for Chinese and English speech input on Linux. The package
 bundles the Python runtime dependencies and a default faster-whisper model.
EOF

chmod -R go-w "$ROOTFS"
find "$ROOTFS" -type d -exec chmod 0755 {} +

desktop-file-validate "$ROOTFS/usr/share/applications/local-speak-input.desktop"
dpkg-deb --build --root-owner-group "$ROOTFS" "$DEB_PATH"

printf 'Built %s\n' "$DEB_PATH"
