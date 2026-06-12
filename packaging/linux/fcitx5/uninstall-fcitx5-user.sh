#!/usr/bin/env bash
set -euo pipefail

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
rm -f "$data_home/fcitx5/addon/voxflow.conf"
rm -f "$data_home/fcitx5/inputmethod/voxflow.conf"
rm -f "$data_home/fcitx5/voxflow/voxflow.so"
rmdir "$data_home/fcitx5/voxflow" 2>/dev/null || true

echo "removed user-level VoxFlow Fcitx5 metadata"
