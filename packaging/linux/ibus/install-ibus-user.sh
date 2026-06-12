#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  cat <<'USAGE'
Usage: install-ibus-user.sh /absolute/path/to/voxflow-ibus

Install the VoxFlow IBus component XML into the current user's IBus
component directory. The engine binary path must not contain spaces.
USAGE
  exit 0
fi

engine_binary="${1:-}"
if [[ -z "$engine_binary" ]]; then
  if command -v voxflow-ibus >/dev/null 2>&1; then
    engine_binary="$(command -v voxflow-ibus)"
  else
    echo "error: pass the voxflow-ibus binary path" >&2
    exit 2
  fi
fi

if [[ "$engine_binary" != /* ]]; then
  echo "error: engine path must be absolute: $engine_binary" >&2
  exit 2
fi
if [[ "$engine_binary" == *" "* ]]; then
  echo "error: IBus component exec path cannot contain spaces: $engine_binary" >&2
  exit 2
fi
if [[ ! -x "$engine_binary" ]]; then
  echo "error: engine is not executable: $engine_binary" >&2
  exit 2
fi

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
component_dir="$data_home/ibus/component"
component_path="$component_dir/voxflow.xml"
mkdir -p "$component_dir"

"$engine_binary" component-xml "$engine_binary --ibus-engine" > "$component_path"

if command -v ibus >/dev/null 2>&1; then
  if ! ibus write-cache; then
    echo "warning: ibus write-cache failed; restart IBus or log out and back in manually" >&2
  fi
fi

echo "installed $component_path"
echo "restart IBus or log out and back in before selecting VoxFlow."
