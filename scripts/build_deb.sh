#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PACKAGE="voxflow"
VERSION="$(python3 - <<'PY'
import tomllib
with open("pyproject.toml", "rb") as fh:
    print(tomllib.load(fh)["project"]["version"])
PY
)"
ARCH="${VOXFLOW_DEB_ARCH:-amd64}"
BUNDLE_PROFILE="${VOXFLOW_BUNDLE_PROFILE:-qwen3-asr-1.7b}"
BUILD_DIR="$ROOT_DIR/build/deb"
ROOTFS="$BUILD_DIR/rootfs"
VENV_DIR="$ROOTFS/opt/voxflow/venv"
DIST_DIR="$ROOT_DIR/dist"
DEB_PATH="$DIST_DIR/${PACKAGE}_${VERSION}_${ARCH}.deb"

case "$BUNDLE_PROFILE" in
  qwen3-asr-1.7b)
    MODEL_REPO="${VOXFLOW_BUNDLE_MODEL_REPO:-Qwen/Qwen3-ASR-1.7B}"
    MODEL_NAME="${VOXFLOW_BUNDLE_MODEL_NAME:-Qwen3-ASR-1.7B}"
    ASR_BACKEND="qwen"
    VENV_EXTRAS=".[qwen,mic]"
    ;;
  qwen3-asr-0.6b)
    MODEL_REPO="${VOXFLOW_BUNDLE_MODEL_REPO:-Qwen/Qwen3-ASR-0.6B}"
    MODEL_NAME="${VOXFLOW_BUNDLE_MODEL_NAME:-Qwen3-ASR-0.6B}"
    ASR_BACKEND="qwen"
    VENV_EXTRAS=".[qwen,mic]"
    ;;
  bundled-faster-whisper-tiny)
    MODEL_REPO=""
    MODEL_NAME="faster-whisper-tiny"
    ASR_BACKEND="faster-whisper"
    VENV_EXTRAS=".[mic]"
    ;;
  *)
    printf 'Unknown VOXFLOW_BUNDLE_PROFILE: %s\n' "$BUNDLE_PROFILE" >&2
    exit 2
    ;;
esac
MODEL_DIR="$ROOTFS/opt/voxflow/models/$MODEL_NAME"
MODEL_CACHE_DIR="${VOXFLOW_MODEL_CACHE_DIR:-$ROOT_DIR/downloads/models/$MODEL_NAME}"

rm -rf "$BUILD_DIR"
mkdir -p "$ROOTFS/DEBIAN" "$DIST_DIR"
mkdir -p "$ROOTFS/opt/voxflow" "$ROOTFS/usr/bin"
mkdir -p "$ROOTFS/etc/voxflow"
mkdir -p "$ROOTFS/usr/share/applications"
mkdir -p "$ROOTFS/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$ROOTFS/usr/share/metainfo"
mkdir -p "$ROOTFS/usr/share/doc/$PACKAGE"
mkdir -p "$ROOTFS/usr/lib/systemd/user"
mkdir -p "$ROOTFS/usr/share/ibus/component"

uv venv --relocatable --python python3 "$VENV_DIR"
uv pip install --python "$VENV_DIR/bin/python" --link-mode copy "$VENV_EXTRAS"

if [ -n "$MODEL_REPO" ]; then
  mkdir -p "$MODEL_CACHE_DIR"
  "$VENV_DIR/bin/python" - <<PY
from huggingface_hub import snapshot_download
snapshot_download(
    repo_id="$MODEL_REPO",
    local_dir="$MODEL_CACHE_DIR",
    local_dir_use_symlinks=False,
)
PY
  mkdir -p "$MODEL_DIR"
  cp -a "$MODEL_CACHE_DIR"/. "$MODEL_DIR"/
else
  mkdir -p "$MODEL_DIR"
  cp -a src/voxflow/bundled/faster-whisper-tiny/. "$MODEL_DIR/"
fi
rm -rf "$MODEL_DIR/.cache"

install -m 0755 packaging/scripts/voxflow "$ROOTFS/usr/bin/voxflow"
install -m 0755 packaging/scripts/voxflow-gui "$ROOTFS/usr/bin/voxflow-gui"
install -m 0755 packaging/scripts/voxflow-daemon "$ROOTFS/usr/bin/voxflow-daemon"
install -m 0755 packaging/scripts/voxflow-tray "$ROOTFS/usr/bin/voxflow-tray"
install -m 0755 packaging/scripts/voxflow-ibus-engine "$ROOTFS/usr/bin/voxflow-ibus-engine"
install -m 0755 packaging/scripts/local-speak "$ROOTFS/usr/bin/local-speak"
install -m 0755 packaging/scripts/local-speak-gui "$ROOTFS/usr/bin/local-speak-gui"
install -m 0755 packaging/scripts/local-speak-daemon "$ROOTFS/usr/bin/local-speak-daemon"
install -m 0755 packaging/scripts/local-speak-tray "$ROOTFS/usr/bin/local-speak-tray"
install -m 0644 packaging/debian/config.toml "$ROOTFS/etc/voxflow/config.toml"
"$VENV_DIR/bin/python" - <<PY
from pathlib import Path
path = Path("$ROOTFS/etc/voxflow/config.toml")
text = path.read_text(encoding="utf-8")
text = text.replace('backend = "faster-whisper"', 'backend = "$ASR_BACKEND"', 1)
text = text.replace('model = "bundled:faster-whisper-tiny"', 'model = "/opt/voxflow/models/$MODEL_NAME"', 1)
path.write_text(text, encoding="utf-8")
PY
install -m 0644 packaging/debian/voxflow.desktop "$ROOTFS/usr/share/applications/voxflow.desktop"
install -m 0644 packaging/icons/voxflow.svg "$ROOTFS/usr/share/icons/hicolor/scalable/apps/voxflow.svg"
install -m 0644 packaging/debian/voxflow.metainfo.xml "$ROOTFS/usr/share/metainfo/voxflow.metainfo.xml"
install -m 0644 packaging/systemd/voxflow.service "$ROOTFS/usr/lib/systemd/user/voxflow.service"
install -m 0644 packaging/ibus/voxflow.xml "$ROOTFS/usr/share/ibus/component/voxflow.xml"
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
Maintainer: Jiaming Wang <w_jming@outlook.com>
Depends: python3 (>= 3.11), python3-gi, ibus, gir1.2-ibus-1.0, gir1.2-gtk-3.0, gir1.2-appindicator3-0.1, gnome-shell-extension-appindicator, libportaudio2, libx11-6, libnotify-bin, ffmpeg, pipewire-bin, wireplumber, xdotool, xclip, xdg-utils, ca-certificates
Provides: local-speak-input
Conflicts: local-speak-input
Replaces: local-speak-input
Installed-Size: $(du -sk "$ROOTFS" | cut -f1)
Description: VoxFlow Input voice input method for Linux
 VoxFlow Input provides a local web console, desktop launcher, top-bar
 indicator, configurable background hotkey daemon, desktop notifications, and
 command-line tools for Chinese and English speech input on Linux. The package
 bundles the Python runtime dependencies and the selected ASR model.
EOF

chmod -R go-w "$ROOTFS"
find "$ROOTFS" -type d -exec chmod 0755 {} +

desktop-file-validate "$ROOTFS/usr/share/applications/voxflow.desktop"
dpkg-deb --build --root-owner-group "$ROOTFS" "$DEB_PATH"

printf 'Built %s\n' "$DEB_PATH"
