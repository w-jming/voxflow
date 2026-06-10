#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

VERSION="$(python3 - <<'PY'
import tomllib
with open("pyproject.toml", "rb") as fh:
    print(tomllib.load(fh)["project"]["version"])
PY
)"
ARCH="${VOXFLOW_PORTABLE_ARCH:-amd64}"
INCLUDE_QWEN_RUNTIME="${VOXFLOW_INCLUDE_QWEN_RUNTIME:-0}"
BUILD_DIR="$ROOT_DIR/build/portable"
APP_NAME="voxflow-$VERSION"
APP_DIR="$BUILD_DIR/$APP_NAME"
DIST_DIR="$ROOT_DIR/dist"
TAR_PATH="$DIST_DIR/${APP_NAME}_${ARCH}.tar.gz"

if [ "$INCLUDE_QWEN_RUNTIME" = "1" ]; then
  VENV_EXTRAS=".[qwen,mic]"
else
  VENV_EXTRAS=".[mic]"
fi

rm -rf "$BUILD_DIR"
mkdir -p "$APP_DIR/bin" "$APP_DIR/share/applications" "$APP_DIR/share/icons" "$APP_DIR/share/ibus/component"
mkdir -p "$APP_DIR/share/doc/voxflow" "$DIST_DIR"

uv venv --relocatable --python python3 "$APP_DIR/venv"
uv pip install --python "$APP_DIR/venv/bin/python" --link-mode copy "$VENV_EXTRAS"
find "$APP_DIR/venv" -path '*/voxflow-*.dist-info/direct_url.json' -delete

install -m 0644 packaging/icons/voxflow.svg "$APP_DIR/share/icons/voxflow.svg"
install -m 0644 README.md "$APP_DIR/share/doc/voxflow/README.md"
install -m 0644 docs/architecture.md "$APP_DIR/share/doc/voxflow/architecture.md"
install -m 0644 docs/linux-setup.md "$APP_DIR/share/doc/voxflow/linux-setup.md"
install -m 0644 docs/model-research.md "$APP_DIR/share/doc/voxflow/model-research.md"
install -m 0644 docs/packaging.md "$APP_DIR/share/doc/voxflow/packaging.md"
install -m 0644 docs/test-report.md "$APP_DIR/share/doc/voxflow/test-report.md"
install -m 0644 packaging/debian/copyright "$APP_DIR/share/doc/voxflow/copyright"

cat >"$APP_DIR/bin/voxflow" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export PYTHONNOUSERSITE=1
exec "$APP_DIR/venv/bin/python" -m voxflow "$@"
SH

cat >"$APP_DIR/bin/voxflow-daemon" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export PYTHONNOUSERSITE=1
exec "$APP_DIR/venv/bin/python" -m voxflow daemon "$@"
SH

cat >"$APP_DIR/bin/voxflow-gui" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
SITE_PACKAGES="$(find "$APP_DIR/venv/lib" -path '*/site-packages' -type d | head -n 1)"
if [ -z "$SITE_PACKAGES" ]; then
  echo "找不到 voxflow Python 包目录。" >&2
  exit 1
fi
export PYTHONPATH="${SITE_PACKAGES}${PYTHONPATH:+:$PYTHONPATH}"
export VOXFLOW_PYTHON="$APP_DIR/venv/bin/python"
export PYTHONNOUSERSITE=1
exec python3 -m voxflow.native_gui "$@"
SH

cat >"$APP_DIR/bin/voxflow-ibus-engine" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
SITE_PACKAGES="$(find "$APP_DIR/venv/lib" -path '*/site-packages' -type d | head -n 1)"
if [ -z "$SITE_PACKAGES" ]; then
  echo "找不到 voxflow Python 包目录。" >&2
  exit 1
fi
export PYTHONPATH="${SITE_PACKAGES}${PYTHONPATH:+:$PYTHONPATH}"
export VOXFLOW_PYTHON="$APP_DIR/venv/bin/python"
export PYTHONNOUSERSITE=1
exec python3 -m voxflow.ibus_engine "$@"
SH

cat >"$APP_DIR/bin/voxflow-tray" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
SITE_PACKAGES="$(find "$APP_DIR/venv/lib" -path '*/site-packages' -type d | head -n 1)"
if [ -z "$SITE_PACKAGES" ]; then
  echo "找不到 voxflow Python 包目录。" >&2
  exit 1
fi
export PYTHONPATH="${SITE_PACKAGES}${PYTHONPATH:+:$PYTHONPATH}"
export VOXFLOW_PYTHON="$APP_DIR/venv/bin/python"
export PYTHONNOUSERSITE=1
exec python3 -m voxflow.tray "$@"
SH

cat >"$APP_DIR/bin/voxflow-install-desktop" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
mkdir -p "$DESKTOP_DIR" "$ICON_DIR"
install -m 0644 "$APP_DIR/share/icons/voxflow.svg" "$ICON_DIR/voxflow.svg"
cat >"$DESKTOP_DIR/voxflow.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=声流输入法
Name[en]=VoxFlow Input
Comment=Chinese and English voice input for Linux
Exec=$APP_DIR/bin/voxflow-gui
Icon=voxflow
Terminal=false
Categories=Utility;AudioVideo;
StartupNotify=true
EOF
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
fi
printf '已安装桌面入口：%s\n' "$DESKTOP_DIR/voxflow.desktop"
SH

cat >"$APP_DIR/bin/voxflow-install-ibus" <<'SH'
#!/bin/sh
set -eu
APP_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
COMPONENT_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/ibus/component"
mkdir -p "$COMPONENT_DIR"
"$APP_DIR/venv/bin/python" - <<PY
from pathlib import Path
from voxflow.ibus_engine import write_component_xml
write_component_xml(Path("$COMPONENT_DIR/voxflow.xml"), exec_path="$APP_DIR/bin/voxflow-ibus-engine")
PY
if command -v ibus >/dev/null 2>&1; then
  ibus restart >/dev/null 2>&1 || true
fi
printf '已安装 IBus component：%s\n' "$COMPONENT_DIR/voxflow.xml"
SH

chmod 0755 "$APP_DIR"/bin/*
tar -C "$BUILD_DIR" -czf "$TAR_PATH" "$APP_NAME"

printf 'Built %s\n' "$TAR_PATH"
