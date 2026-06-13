#!/usr/bin/env bash
# 把 VoxFlow IBus 引擎安装到当前用户会话(无需 sudo):
# component XML → ~/.local/share/ibus/component/,注册输入源,重启 ibus。
set -euo pipefail

ENGINE_BIN="${1:-$HOME/software/voxflow/bin/voxflow-ibus}"
COMPONENT_DIR="$HOME/.local/share/ibus/component"

if [ ! -x "$ENGINE_BIN" ]; then
  echo "engine binary not found: $ENGINE_BIN" >&2
  exit 1
fi

mkdir -p "$COMPONENT_DIR"
"$ENGINE_BIN" component-xml "$ENGINE_BIN --ibus-engine" > "$COMPONENT_DIR/voxflow.xml"
echo "component installed: $COMPONENT_DIR/voxflow.xml"

# 注册为 GNOME 输入源(幂等)
if command -v gsettings >/dev/null 2>&1; then
  current=$(gsettings get org.gnome.desktop.input-sources sources)
  if [[ "$current" != *"'voxflow'"* ]]; then
    new=$(python3 - "$current" <<'PY'
import ast, sys
sources = ast.literal_eval(sys.argv[1])
sources.append(('ibus', 'voxflow'))
print(repr(sources))
PY
)
    gsettings set org.gnome.desktop.input-sources sources "$new"
    echo "input source registered: $new"
  else
    echo "input source already registered"
  fi
fi

ibus restart >/dev/null 2>&1 || ibus-daemon -drx
sleep 2
if ibus list-engine 2>/dev/null | grep -qi voxflow; then
  echo "✓ ibus engine visible"
else
  cat <<MSG
⚠ IBus($(ibus version 2>/dev/null))只扫描系统组件目录,需要一次 sudo 安装:

    sudo cp "$COMPONENT_DIR/voxflow.xml" /usr/share/ibus/component/ && ibus restart

(免 sudo 的引擎自注册方案在 todo,下批次实现)
MSG
fi

cat <<'EOF'

使用方法:
  1. 确保 voxflow-core 守护进程在运行(~/software/voxflow/run-core.sh)
  2. 用 Super+Space 切换输入源到「VoxFlow / 声流输入法」(或 ibus engine voxflow)
  3. 聚焦任意输入框,按 Alt+S 开始/停止听写
EOF
