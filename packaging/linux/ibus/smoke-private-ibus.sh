#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /absolute/path/to/voxflow-ibus" >&2
  exit 2
fi

engine_bin="$1"
if [[ "$engine_bin" != /* || ! -x "$engine_bin" ]]; then
  echo "error: engine path must be an absolute executable path: $engine_bin" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
core_bin="${VOXFLOW_CORE_BIN:-$repo_root/target/debug/voxflow-core}"
if [[ ! -x "$core_bin" ]]; then
  echo "error: voxflow-core binary is not executable: $core_bin" >&2
  exit 2
fi

base="${VOXFLOW_IBUS_SMOKE_TMP:-$(mktemp -d /tmp/voxflow-ibus-private.XXXXXX)}"
core_pid=""

cleanup() {
  if [[ -n "$core_pid" ]]; then
    kill "$core_pid" 2>/dev/null || true
    wait "$core_pid" 2>/dev/null || true
  fi
  if [[ -z "${VOXFLOW_IBUS_SMOKE_KEEP:-}" ]]; then
    rm -rf "$base"
  else
    echo "kept smoke directory: $base" >&2
  fi
}
trap cleanup EXIT

mkdir -p "$base"
export VOXFLOW_HOME="$base/home"
export XDG_DATA_HOME="$base/data"
export XDG_CACHE_HOME="$base/cache"
export XDG_CONFIG_HOME="$base/config"
export XDG_RUNTIME_DIR="$base/runtime"
mkdir -p "$VOXFLOW_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_CONFIG_HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export IBUS_COMPONENT_PATH="$XDG_DATA_HOME/ibus/component"

"$script_dir/install-ibus-user.sh" "$engine_bin" >/dev/null

"$core_bin" serve >"$base/voxflow-core.log" 2>&1 &
core_pid="$!"

core_socket="$XDG_RUNTIME_DIR/voxflow/core.sock"
export VOXFLOW_CORE_SOCKET="$core_socket"
export VOXFLOW_IBUS_ENGINE_BIN="$engine_bin"
for _ in $(seq 1 50); do
  if [[ -S "$core_socket" ]]; then
    break
  fi
  sleep 0.1
done
if [[ ! -S "$core_socket" ]]; then
  echo "error: Core socket was not created: $core_socket" >&2
  cat "$base/voxflow-core.log" >&2 || true
  exit 1
fi

dbus-run-session -- bash -c '
set -euo pipefail
ibus_daemon_pid=""
voxflow_ibus_pid=""
cleanup_ibus() {
  if [[ -n "$voxflow_ibus_pid" ]]; then
    kill "$voxflow_ibus_pid" 2>/dev/null || true
    wait "$voxflow_ibus_pid" 2>/dev/null || true
  fi
  if [[ -n "$ibus_daemon_pid" ]]; then
    kill "$ibus_daemon_pid" 2>/dev/null || true
    wait "$ibus_daemon_pid" 2>/dev/null || true
  fi
}
trap cleanup_ibus EXIT
ibus-daemon -s --panel=disable --config=disable --emoji-extension=disable --cache=refresh &
ibus_daemon_pid="$!"
for _ in $(seq 1 50); do
  address_file="$(find "$XDG_CONFIG_HOME/ibus/bus" -type f -print -quit 2>/dev/null || true)"
  if [[ -n "$address_file" ]]; then
    # shellcheck disable=SC1090
    source "$address_file"
    export IBUS_ADDRESS
    break
  fi
  sleep 0.1
done
if [[ -z "${IBUS_ADDRESS:-}" ]]; then
  echo "error: private ibus-daemon did not publish IBUS_ADDRESS" >&2
  exit 1
fi
for _ in $(seq 1 50); do
  if ibus list-engine | grep -Fq "voxflow"; then
    break
  fi
  sleep 0.1
done
ibus list-engine | grep -F "voxflow"
if ibus engine voxflow; then
  echo "private IBus SetGlobalEngine accepted voxflow"
else
  echo "warning: headless IBus SetGlobalEngine failed; continuing with direct Factory smoke" >&2
fi
wait_for_voxflow_bus_name() {
  for _ in $(seq 1 50); do
    if busctl --user --no-pager list | grep -Fq "org.freedesktop.IBus.VoxFlow"; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}
if ! wait_for_voxflow_bus_name; then
  "$VOXFLOW_IBUS_ENGINE_BIN" --ibus-engine --core-socket "$VOXFLOW_CORE_SOCKET" &
  voxflow_ibus_pid="$!"
fi
for _ in $(seq 1 50); do
  if busctl --user --no-pager list | grep -Fq "org.freedesktop.IBus.VoxFlow"; then
    break
  fi
  sleep 0.1
done
busctl --user call org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Factory org.freedesktop.IBus.Factory CreateEngine s voxflow
busctl --user introspect org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Engine/VoxFlow/voxflow/1 org.freedesktop.IBus.Engine >/dev/null
busctl --user call org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Engine/VoxFlow/voxflow/1 org.freedesktop.IBus.Engine FocusIn
busctl --user call org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Engine/VoxFlow/voxflow/1 org.freedesktop.IBus.Engine FocusOut
'

echo "private IBus smoke passed"
