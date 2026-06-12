# Windows 迁移设计

> **编号** VF-PLT-04 · **版本** 0.2 · **状态** 草案(P2 预研) · **最后更新** 2026-06-10

本文档为预研(spike)范围:目标是验证可行性并回填本档,不是实现承诺。预研工期预估 3-4 周(TSF 复杂度高于 IMK)。

## 1. 目标

Windows 版本使用 Text Services Framework(TSF)作为主输入路径,实现系统输入法级 composition 和 commit。

## 2. 输入法

```text
TSF Text Service (C++/Rust COM 组件)
  -> named pipe -> voxflow-core -> streaming ASR
```

TSF 前端职责:composition、commit、surrounding text 读取、账本范围内的删除/替换。业务逻辑留在 Core,COM 组件只做协议翻译。

## 3. 音频

WASAPI(`AudioBackend` Windows 实现):默认输入设备、设备枚举、音量电平、蓝牙耳机输入状态(免提 profile 检测)。

## 4. 托盘与控制台

Tauri 提供控制台与托盘(Shell NotifyIcon);Core 以用户会话进程或服务运行(倾向用户会话,避免服务跨会话复杂度)。

## 5. 打包

候选 MSIX(现代,商店可选)或 NSIS(灵活)。需要:代码签名证书、TSF 组件注册(regsvr / MSIX 清单)、卸载时清理注册项与输入法列表。

## 6. 预研需要回答的问题

1. TSF text service 的最小可用实现量级(COM 接口面有多大)。
2. Rust 直接实现 COM(windows-rs)还是 C++ 薄壳的成本对比。
3. 注册输入服务是否必须管理员权限;MSIX 能否免提权安装。
4. 主流应用(Office、浏览器、终端、Electron)对第三方 TSF composition 的兼容矩阵。

## 7. 风险

- TSF 是三个平台中开发复杂度最高的输入法框架。
- 不同应用对 composition 支持差异明显,降级矩阵必须前置设计。
- 代码签名与 SmartScreen 信誉积累周期长,影响发布初期安装体验。
