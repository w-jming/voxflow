#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  cat <<'USAGE'
Usage: install-fcitx5-user.sh /absolute/path/to/voxflow.so [voxflow-fcitx5]

Install VoxFlow Fcitx5 metadata into the current user's Fcitx5 data
directory. The first argument must be a real Fcitx5 addon shared library.
USAGE
  exit 0
fi

addon_library="${1:-}"
metadata_tool="${2:-}"
if [[ -z "$addon_library" ]]; then
  echo "error: pass the voxflow Fcitx5 addon .so path" >&2
  exit 2
fi
if [[ "$addon_library" != /* ]]; then
  echo "error: addon path must be absolute: $addon_library" >&2
  exit 2
fi
if [[ ! -f "$addon_library" ]]; then
  echo "error: addon library does not exist: $addon_library" >&2
  exit 2
fi

if [[ -z "$metadata_tool" ]]; then
  if command -v voxflow-fcitx5 >/dev/null 2>&1; then
    metadata_tool="$(command -v voxflow-fcitx5)"
  else
    echo "error: pass the voxflow-fcitx5 metadata tool path" >&2
    exit 2
  fi
fi
if [[ ! -x "$metadata_tool" ]]; then
  echo "error: metadata tool is not executable: $metadata_tool" >&2
  exit 2
fi

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
addon_dir="$data_home/fcitx5/addon"
inputmethod_dir="$data_home/fcitx5/inputmethod"
lib_dir="$data_home/fcitx5/voxflow"
mkdir -p "$addon_dir" "$inputmethod_dir" "$lib_dir"

install -m 0755 "$addon_library" "$lib_dir/voxflow.so"
"$metadata_tool" addon-conf "$lib_dir/voxflow.so" > "$addon_dir/voxflow.conf"
"$metadata_tool" inputmethod-conf > "$inputmethod_dir/voxflow.conf"

echo "installed $addon_dir/voxflow.conf"
echo "installed $inputmethod_dir/voxflow.conf"
echo "restart Fcitx5 or log out and back in before selecting VoxFlow."
