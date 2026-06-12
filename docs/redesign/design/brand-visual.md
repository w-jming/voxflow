# 品牌视觉设计

> **编号** VF-DSN-01 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

## 1. 品牌

- 英文名:**VoxFlow** 中文名:**声流输入法**
- 定位:清爽、可靠、本地优先的桌面语音输入法。

视觉关键词:清晰、快速、本地、可控、专业。
避免:大面积绿色、花哨渐变、营销式 hero、过度圆角、网页 demo 式布局。

## 2. 色彩

色板唯一来源为 [UI 系统 §2](ui-system.md),本节为品牌层约束(**主题色不变**):

```text
Primary Sky      #0EA5E9   品牌主色
Primary Deep     #0369A1   主按钮底、深色标题
Accent Cyan      #22D3EE   轻强调
Ink              #0F172A   主文本
Text Secondary   #475569   次文本
Surface          #F8FAFC   页面背景
Panel            #FFFFFF   面板
Border           #D7E7F2   边框
Warning          #F59E0B   警告
Error            #DC2626   错误
Success          #2563EB   正常/完成(不用绿色)
```

- 成功状态不使用绿色作为主色,避免整体偏绿;用蓝色表示正常和完成。
- 品牌主色保持 `#0EA5E9`;深色模式使用更亮同色系 `#38BDF8` 增强对比。
- 经 D-16 裁决,色板按同色相补全为主色阶 `50–900`(原色板全部颜色落位其中,色相不变),用于按钮三态等交互细节;阶梯定义见 [UI 系统 §2.1](ui-system.md),对比度规则见[主题系统 §7](theme-system.md)。

## 3. Logo 与 Symbol

概念:一条水平声流曲线 + 一个输入光标。简洁、可缩放。

### 3.1 资产

| 资产 | 用途 |
| --- | --- |
| [voxflow-logo.svg](assets/voxflow-logo.svg) / [深色版](assets/voxflow-logo-dark.svg) | 控制台 header、关于页、文档封面 |
| [voxflow-symbol.svg](assets/voxflow-symbol.svg) / [深色版](assets/voxflow-symbol-dark.svg) | 托盘、启动器、窗口图标、小尺寸状态位 |

### 3.2 使用规则

- 横版 Logo 必须直接复用 Symbol 的同一套路径几何,不允许重新绘制"近似"图标。
- 标志图形最多一条水平声流曲线 + 一个输入光标;不堆叠声波细节,避免字母化误读。
- 16-24px 小图标只使用 Symbol,不带中英文名称。
- 深色背景用深色版,浅色背景用浅色版;浅/深模式不得使用不同形状的 Logo。
- 不拉伸,不加阴影、发光、渐变描边,不加与 Symbol 无关的装饰声波线。

### 3.3 尺寸与安全边距

| 项 | 规则 |
| --- | --- |
| Symbol 最小尺寸 | 16px(再小不可用) |
| 横版 Logo 最小高度 | 24px |
| Logo 安全边距 | 四周 ≥ Symbol 宽度的 25% |
| 托盘图标 | 四周保留 2px 透明边距,避免贴边 |
| GNOME 托盘 | 提供 symbolic 单色变体(`-symbolic.svg`,currentColor),随系统着色 |

## 4. 字体

系统字体优先,不使用夸张 display 字体:

| 平台 | 字体 |
| --- | --- |
| Linux | Noto Sans CJK / system UI |
| macOS | SF Pro / PingFang SC |
| Windows | Segoe UI / Microsoft YaHei UI |

字号与行高体系见 [UI 系统 §2.3](ui-system.md)。

## 5. 组件视觉原则

- 卡片半径 8px;页面背景浅色;主按钮蓝色系、危险按钮红色。
- 状态 badge 使用低饱和底色(token 见 [UI 系统 §2.1](ui-system.md))。
- 图标按钮必须有 tooltip。
- 表格和列表优先紧凑可扫描。
- 加载使用轻量 spinner 或稳定进度条,不改变布局尺寸。

## 6. 控制台首页定位

首页不是说明页,而是**状态工作台**。打开后第一眼看到:VoxFlow 是否正在工作、输入法是否激活、麦克风是否可用、模型是否可用、语义撤销分类器是否可用。详见[控制台规格](control-center-spec.md)。

## 7. 设计资产基准

- [控制台线框图](assets/control-center-wireframe.svg)
- [光标处输入反馈流程](assets/input-preedit-flow.svg)

SVG 是方向性基准,不要求逐像素复刻,但页面信息架构、状态优先级和主色使用必须保持一致。

## 8. 禁止项

1. 不使用顶部通知作为输入中主反馈。
2. 不使用大面积绿色作为正常状态。
3. 不使用装饰性渐变背景。
4. 不把控制台做成只有表单的设置页。
5. 不在普通用户页面显示未实现的后端选项。
6. 不在浅色/深色模式中使用不同形状的 Logo。
