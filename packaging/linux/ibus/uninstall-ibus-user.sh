#!/usr/bin/env bash
set -euo pipefail

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
component_path="$data_home/ibus/component/voxflow.xml"

if [[ -e "$component_path" ]]; then
  rm -f "$component_path"
  echo "removed $component_path"
else
  echo "not installed: $component_path"
fi

if command -v ibus >/dev/null 2>&1; then
  if ! ibus write-cache; then
    echo "warning: ibus write-cache failed; restart IBus or log out and back in manually" >&2
  fi
fi

echo "restart IBus or log out and back in to fully unregister VoxFlow."
