# Debian 打包占位

阶段 7 在此实现新版 deb 打包。

目标安装布局:

```text
/usr/bin/voxflow
/usr/lib/voxflow/voxflow-core
/usr/lib/voxflow/voxflow-ibus
<fcitx5 addon dir>/voxflow.so
/usr/bin/voxflow-control-center
/usr/share/ibus/component/voxflow.xml
/usr/share/fcitx5/addon/voxflow.conf
/usr/share/fcitx5/inputmethod/voxflow.conf
/usr/lib/systemd/user/voxflow-core.service
```
