# 打包与发布

> **编号** VF-ENG-03 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

## 1. 发布目标

每个发布版本必须提供可直接使用的安装包,而不是只提供源码(FR-PKG-*)。

| 平台 | 制品 | 优先级 |
| --- | --- | --- |
| Linux | deb、portable tar | P0 |
| Linux | AppImage | P1 |
| macOS | dmg/pkg | P1 |
| Windows | MSIX 或 NSIS | P2 |

MVP 阶段 Linux 发布**单一 deb 包**(含 core + IBus 引擎 + Fcitx5 addon + 控制台,D-13 裁决);P1 用户群扩大后再评估拆分。

## 2. Linux deb

系统目录只放程序和静态资源:

```text
/usr/bin/voxflow                  CLI 入口(doctor/dictate/status)
/usr/lib/voxflow/voxflow-core     daemon
/usr/lib/voxflow/voxflow-ibus     IBus 引擎
<fcitx5 addon 目录>/voxflow.so    Fcitx5 addon(构建时按发行版探测路径)
/usr/bin/voxflow-control-center   Tauri 控制台(含状态指示器窗口)
/usr/share/ibus/component/voxflow.xml
/usr/share/fcitx5/addon/voxflow.conf
/usr/share/fcitx5/inputmethod/voxflow.conf
/usr/share/applications/voxflow.desktop
/usr/share/icons/hicolor/...      Symbol 多尺寸
/usr/lib/systemd/user/voxflow-core.service
```

大体积和可增长数据一律放用户目录(`~/.voxflow/` 的 models/logs/cache/config,见[模型管理 §2](../architecture/model-management.md))。

维护脚本要求:postinst 触发 `ibus write-cache`(存在时)与图标缓存刷新;postrm 清理注册但**不删除**用户数据目录;卸载后重装不丢配置。

## 3. portable

```text
voxflow-<version>/
  bin/            core、ibus 引擎、控制台、CLI
  lib/  share/
  install-desktop    用户级 .desktop + 图标
  install-ibus       用户级 ibus component 注册
  install-fcitx5     用户级 fcitx5 addon 安装(校验 ABI 版本)
  uninstall          清理上述安装产物
```

不得要求 root;脚本幂等可重复执行;`VOXFLOW_HOME` 未设置时仍用 `~/.voxflow`。

## 4. 发布检查单

发布前必须全部通过(与 [CI 流水线](testing-strategy.md)对应):

1. fmt/clippy/eslint 零警告基线。
2. 单元 + 集成测试全绿。
3. 包测试矩阵(Ubuntu 22.04/24.04、Debian 12 容器)安装/升级/卸载通过。
4. secret scan 零泄漏。
5. license check:依赖与内置模型许可证清单更新。
6. 模型 manifest check:profile 来源、checksum 可达性抽查。
7. 性能基准达标(见[测试策略 §5](testing-strategy.md))。
8. README 安装路径与命令实测一致。
9. 人工验收记录归档(系统/桌面/应用矩阵)。

## 5. 版本与渠道

语义化版本:patch = bugfix;minor = 兼容新功能;major = 架构或配置不兼容变更。

- git tag:`v<semver>`;release 分支:`release/<major.minor>`。
- 渠道:stable(deb/portable)+ beta(预发布 tag,portable 优先)。
- **模型版本与软件版本分离**:模型经 profile `version` 字段管理,软件升级不强制重下模型;分类器包独立 semver(见[分类器规范 §10](../architecture/semantic-intent-classifier.md))。
- 每个 release 附带:变更说明(用户可见行为)、已知问题、升级注意事项。
