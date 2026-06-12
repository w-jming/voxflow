# macOS 迁移设计

> **编号** VF-PLT-03 · **版本** 0.2 · **状态** 草案(P1 预研) · **最后更新** 2026-06-10

本文档为预研(spike)范围:目标是验证可行性并回填本档,不是实现承诺。预研工期预估 2-3 周。

## 1. 目标

macOS 版本使用系统输入法能力(InputMethodKit)作为主路径,不以模拟键盘/辅助功能注入为主路径。

## 2. 输入法

```text
InputMethodKit bundle (Swift/ObjC 薄壳)
  -> UDS -> voxflow-core -> streaming ASR
```

bundle 职责:composition(对应 preedit)、commit、删除最近 VoxFlow 文本、输入源激活状态上报。业务逻辑全部留在 Core,bundle 只做协议翻译(与 IBus 前端同构)。

安装:`~/Library/Input Methods/`;用户需在系统设置中启用输入源,首次启用引导由控制台承担。

## 3. 音频

CoreAudio:麦克风权限(`NSMicrophoneUsageDescription` + TCC 弹窗)、设备切换、采样率转换。`AudioBackend` trait 的 macOS 实现。

## 4. 权限

| 权限 | 是否必需 |
| --- | --- |
| 麦克风 | 必需 |
| 输入法安装(用户目录) | 必需,无需管理员 |
| 辅助功能 | 避免依赖;仅 fallback 注入模式需要 |

## 5. 打包

dmg(控制台 + Core)+ 输入法 bundle;需要 Apple Developer ID 签名与 notarization(年费与证书管理纳入发布成本)。Tauri 对 macOS 打包/签名有现成链路。

## 6. 预研需要回答的问题

1. IMK 第三方输入法在最新 macOS 上的 composition/commit 行为与限制(逐应用矩阵)。
2. daemon(launchd agent)与 IMK bundle 的 UDS 通信是否受沙盒/TCC 限制。
3. 删除已 commit 文本的可行性(IMK 无 delete_surrounding 等价物时,撤销能力的降级档位)。
4. 签名 + notarization 全链路在 CI 中自动化的成本。

## 7. 风险

- InputMethodKit 文档陈旧、行为随版本漂移,预研结论需标注 macOS 版本。
- 用户安装第三方输入法的引导成本高(需逐步截图指引)。
- 与 Linux 共享的能力降级矩阵必须覆盖 IMK 缺失能力,避免撤销功能不一致。
