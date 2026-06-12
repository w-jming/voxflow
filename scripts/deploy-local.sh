#!/usr/bin/env bash
# 把 VoxFlow 部署到 ~/software/voxflow 供真机测试(D-22:默认 Qwen3-ASR-1.7B + vLLM)。
#
# 模型权重不入 git:本脚本经 venv pip 安装 qwen-asr[vllm] 并从 Hugging Face
# 预下载 Qwen/Qwen3-ASR-1.7B 到本地缓存;zipformer 兜底模型经控制中心
# 模型页一键下载(或已有 ~/.voxflow/models)。
#
# 用法: scripts/deploy-local.sh [--skip-python]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY_DIR="${VOXFLOW_DEPLOY_DIR:-$HOME/software/voxflow}"
export PATH="$HOME/.cargo/bin:$PATH"

echo "==> building release binaries"
cargo build --release -p voxflow-core -p voxflow-control-center
( cd "$REPO_DIR/apps/control-center" && npm run build >/dev/null )

echo "==> installing to $DEPLOY_DIR"
mkdir -p "$DEPLOY_DIR/bin" "$DEPLOY_DIR/sidecar"
install -m 755 "$REPO_DIR/target/release/voxflow-core" "$DEPLOY_DIR/bin/"
install -m 755 "$REPO_DIR/target/release/voxflow-control-center" "$DEPLOY_DIR/bin/"
# sherpa 动态库($ORIGIN rpath,zipformer 兜底后端需要)
for lib in "$REPO_DIR"/target/release/*.so; do
  [ -f "$lib" ] && install -m 644 "$lib" "$DEPLOY_DIR/bin/"
done
install -m 644 "$REPO_DIR/sidecar/qwen3_asr_sidecar.py" "$DEPLOY_DIR/sidecar/"

if [[ "${1:-}" != "--skip-python" ]]; then
  echo "==> python venv + qwen-asr[vllm] + 模型权重(已存在则跳过)"
  export PATH="$HOME/.local/bin:$PATH"
  if [ ! -x "$DEPLOY_DIR/venv/bin/python" ]; then
    if command -v uv >/dev/null 2>&1; then
      uv venv "$DEPLOY_DIR/venv"
    else
      python3 -m venv "$DEPLOY_DIR/venv"   # 需要 python3-venv 包
    fi
  fi
  if command -v uv >/dev/null 2>&1; then
    uv pip install --python "$DEPLOY_DIR/venv/bin/python" "qwen-asr[vllm]" "huggingface_hub[cli]"
  else
    "$DEPLOY_DIR/venv/bin/pip" install -q "qwen-asr[vllm]" "huggingface_hub[cli]"
  fi
  "$DEPLOY_DIR/venv/bin/python" - <<'PY'
from huggingface_hub import snapshot_download
path = snapshot_download("Qwen/Qwen3-ASR-1.7B")
print(f"weights ready: {path}")
PY
fi

echo "==> writing user config (~/.voxflow/config.toml) — 仅当 asr 段缺失时追加"
mkdir -p "$HOME/.voxflow"
CONFIG="$HOME/.voxflow/config.toml"
if [ ! -f "$CONFIG" ] || ! grep -q "^\[asr\]" "$CONFIG"; then
  cat >> "$CONFIG" <<EOF

[asr]
backend = "qwen3_vllm"

[asr.qwen3]
python = "$DEPLOY_DIR/venv/bin/python"
sidecar_script = "$DEPLOY_DIR/sidecar/qwen3_asr_sidecar.py"
model = "Qwen/Qwen3-ASR-1.7B"
EOF
fi

echo "==> launch scripts + README"
cat > "$DEPLOY_DIR/run-core.sh" <<EOF
#!/usr/bin/env bash
exec "$DEPLOY_DIR/bin/voxflow-core" serve
EOF
cat > "$DEPLOY_DIR/run-control-center.sh" <<EOF
#!/usr/bin/env bash
exec "$DEPLOY_DIR/bin/voxflow-control-center"
EOF
chmod +x "$DEPLOY_DIR"/run-*.sh

cat > "$DEPLOY_DIR/README.md" <<'EOF'
# VoxFlow 本地部署(真机测试)

1. 启动 Core: `./run-core.sh`(保持运行)
2. 启动控制中心: `./run-control-center.sh`
3. 「输入」页选择语音识别后端:
   - Qwen3-ASR-1.7B + vLLM(默认):首次开始听写加载模型约 1 分钟(GPU)
   - 火山引擎 API:填入 APP ID / Access Token 后切换
   - Zipformer 本地:先在「模型」页一键下载安装
4. 「模型」页可下载/导入/激活本地模型(全部存放于 ~/.voxflow,不碰系统目录)。

权重缓存:~/.cache/huggingface(Qwen3-ASR-1.7B 已预下载)。
配置与日志:~/.voxflow/。
EOF

echo "==> done: $DEPLOY_DIR"
