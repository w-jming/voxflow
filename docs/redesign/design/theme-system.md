# 主题系统设计

> **编号** VF-DSN-03 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

## 1. 目标

控制台必须支持浅色、深色和跟随系统(FR-CC-09)。主题切换是 P0 UI 能力,直接影响长时间听写时的可用性、夜间体验和系统一致性。**色板不变**:全部颜色值以 [UI 系统 §2](ui-system.md) 为唯一来源,本文档只定义切换机制。

## 2. 模式

| 模式 | 配置值 | 行为 |
| --- | --- | --- |
| 跟随系统 | `system` | 使用操作系统当前外观,系统变化时自动切换 |
| 浅色 | `light` | 强制浅色 |
| 深色 | `dark` | 强制深色 |

默认值:`system`。

## 3. 配置与 IPC

```toml
[ui]
theme = "system"      # system | light | dark
reduce_motion = false
```

更新经 `config.update`(见 [IPC §3.3](../architecture/ipc-api.md)):

```json
{ "patch": { "ui": { "theme": "dark" } } }
```

Core 负责持久化并广播 `config.changed`,使多个 UI 入口(控制台、托盘菜单)保持一致;主题的**应用**完全在 UI 层完成。

## 4. 运行时行为

主题切换必须:

- 即时更新 CSS variables 与 Logo/Symbol 资产。
- 不重启 Core、不重启输入法前端、不中断当前听写会话、不清空模型下载进度。
- 切换过渡 ≤ `--motion-base`(160ms),`reduce_motion` 时无过渡。

## 5. 前端实现

启动序列(防白屏闪烁/FOUC):

1. Tauri 窗口创建时按上次主题设置原生背景色(`#F8FAFC` / `#020617`),避免 WebView 加载前闪白。
2. HTML 内联首屏脚本:读本地缓存的主题值,先设 `<html data-theme="…">`再加载样式。
3. 连接 Core 后以 `config.get` 的 `ui.theme` 为准校正。
4. `system` 模式监听 `prefers-color-scheme` 变化。
5. 用户手动切换时调用 `config.update` 持久化。

```css
:root[data-theme="light"] {
  --vf-surface: #f8fafc; --vf-panel: #ffffff; --vf-ink: #0f172a;
  --vf-muted: #475569; --vf-border: #d7e7f2;
  --vf-primary: #0ea5e9; --vf-primary-deep: #0369a1;
  --vf-btn-bg: var(--vf-primary-700);
  --vf-btn-bg-hover: var(--vf-primary-800);
  --vf-btn-bg-active: var(--vf-primary-900);
}

:root[data-theme="dark"] {
  --vf-surface: #020617; --vf-panel: #0f172a; --vf-ink: #f8fafc;
  --vf-muted: #cbd5e1; --vf-border: #1e3a5f;
  --vf-primary: #38bdf8; --vf-primary-deep: #7dd3fc;
  --vf-btn-bg: var(--vf-primary-400);
  --vf-btn-bg-hover: var(--vf-primary-300);
  --vf-btn-bg-active: var(--vf-primary-500);
}
```

主色阶 `--vf-primary-50…900` 为主题无关常量(见 [UI 系统 §2.1](ui-system.md)),在 `:root` 定义一次,语义 token 按主题引用阶梯档位。

(完整 token 见 [UI 系统 §2](ui-system.md);组件一律引用 token,禁止硬编码色值。)

## 6. 品牌资产

| 主题 | Logo | Symbol |
| --- | --- | --- |
| 浅色 | [voxflow-logo.svg](assets/voxflow-logo.svg) | [voxflow-symbol.svg](assets/voxflow-symbol.svg) |
| 深色 | [voxflow-logo-dark.svg](assets/voxflow-logo-dark.svg) | [voxflow-symbol-dark.svg](assets/voxflow-symbol-dark.svg) |

品牌几何必须一致:深色版只允许改变颜色,不允许改变图形形状、比例或路径。托盘图标随系统托盘主题选择对应版本;GNOME 下提供 symbolic 单色变体(见[品牌视觉 §4](brand-visual.md))。

## 7. 对比度对照表

以下为计算估值(WCAG 2.1 相对亮度公式),实现时由 CI 内的自动对比度检查复核:

| 前景 / 背景 | 浅色 | 深色 | 要求 |
| --- | --- | --- | --- |
| 主文本 ink / surface | ≈ 17:1 ✓ | ≈ 17:1 ✓ | ≥ 4.5:1 |
| 次文本 muted / surface | ≈ 7:1 ✓ | ≈ 12:1 ✓ | ≥ 4.5:1 |
| 主按钮·默认 | 白字 / `700 #0369A1` ≈ 5.9:1 ✓ | ink 字 / `400 #38BDF8` ≈ 8:1 ✓ | ≥ 4.5:1 |
| 主按钮·hover | 白字 / `800 #075985` ≈ 7.6:1 ✓ | ink 字 / `300 #7DD3FC` ≈ 10.7:1 ✓ | ≥ 4.5:1 |
| 主按钮·active | 白字 / `900 #0C4A6E` ≈ 10:1 ✓ | ink 字 / `500 #0EA5E9` ≈ 6.4:1 ✓ | ≥ 4.5:1 |
| 白字 / `500 #0EA5E9` | ≈ 2.8:1 ✗(故不用作小字按钮底) | — | — |
| badge 字 / badge 底 | `#0369A1`/`#E0F2FE` ≈ 6:1 ✓ | 亮字/深底 ≥ 4.5:1 | ≥ 4.5:1 |

用法规则:主按钮三态使用主色阶 `700/800/900`(浅色)与 `400/300/500`(深色);品牌主色 `500 #0EA5E9` 用于选中态、焦点环、图形、大号文本(≥ 18.66px bold 时 3:1 即可)。主色阶定义与 D-16 裁决见 [UI 系统 §2.1](ui-system.md)。

## 8. 可访问性与测试

可访问性:错误/警告/正常不能只靠颜色区分;支持减少动效;主题切换控件有文本标签和 tooltip。

必须测试(对应[测试策略 §4](../engineering/testing-strategy.md)):

- 三种主题初始加载,无 FOUC。
- 切换即时生效;`system` 模式跟随系统变化。
- Logo/Symbol 随主题切换。
- 切换期间听写会话与模型下载不中断(IT)。
- 深色主题下图表、进度条、badge、表单全部可读(对比度自动检查)。
