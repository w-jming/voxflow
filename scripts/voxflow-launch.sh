#!/usr/bin/env bash
# VoxFlow 一键启动:确保 Core 守护进程在运行,然后打开控制中心。
# 由桌面快捷方式(voxflow.desktop)调用。第二次启动经单实例插件聚焦已有窗口。
set -u

DIR="${VOXFLOW_DEPLOY_DIR:-$HOME/software/voxflow}"
LOGS="$HOME/.voxflow/logs"
mkdir -p "$LOGS"

if ! pgrep -f "$DIR/bin/voxflow-core serve" >/dev/null 2>&1; then
  setsid "$DIR/bin/voxflow-core" serve >"$LOGS/core-daemon.log" 2>&1 < /dev/null &
  sleep 1
fi

exec "$DIR/bin/voxflow-control-center"
