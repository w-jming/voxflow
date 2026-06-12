# Portable 打包占位

阶段 7 在此实现 portable tar。

目标布局:

```text
voxflow-<version>/
  bin/
  lib/
  share/
  install-desktop
  install-ibus
  install-fcitx5
  uninstall
```

portable 不得要求 root;安装和卸载脚本必须幂等。
