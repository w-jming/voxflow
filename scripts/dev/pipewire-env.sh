#!/usr/bin/env bash
# 无 sudo 环境下构建 voxflow-audio `pipewire-native` feature 的开发辅助:
# 用 apt-get download + dpkg -x 把 libpipewire-0.3-dev / libspa-0.2-dev 提取到
# ~/.local/pipewire-dev,并导出 pkg-config 环境。有 sudo 的机器直接
# `sudo apt install libpipewire-0.3-dev libspa-0.2-dev` 即可,无需本脚本。
#
# 用法: source scripts/dev/pipewire-env.sh
set -u

PIPEWIRE_DEV_ROOT="${PIPEWIRE_DEV_ROOT:-$HOME/.local/pipewire-dev}"

if pkg-config --exists libpipewire-0.3 2>/dev/null; then
    echo "system libpipewire-0.3 dev files already available; nothing to do"
    return 0 2>/dev/null || exit 0
fi

if [ ! -f "$PIPEWIRE_DEV_ROOT/extracted/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" ]; then
    mkdir -p "$PIPEWIRE_DEV_ROOT"
    (
        cd "$PIPEWIRE_DEV_ROOT" || exit 1
        apt-get download libpipewire-0.3-dev libspa-0.2-dev || exit 1
        for deb in ./*.deb; do dpkg -x "$deb" extracted/; done
        # dev 包里的 .so 符号链接指向同目录,真实运行库在系统路径
        ln -sf /lib/x86_64-linux-gnu/libpipewire-0.3.so.0 \
            extracted/usr/lib/x86_64-linux-gnu/libpipewire-0.3.so
    ) || { echo "pipewire dev extraction failed" >&2; return 1 2>/dev/null || exit 1; }
fi

export PKG_CONFIG_SYSROOT_DIR="$PIPEWIRE_DEV_ROOT/extracted"
export PKG_CONFIG_PATH="$PIPEWIRE_DEV_ROOT/extracted/usr/lib/x86_64-linux-gnu/pkgconfig"
echo "pipewire dev environment ready: PKG_CONFIG_SYSROOT_DIR=$PKG_CONFIG_SYSROOT_DIR"
