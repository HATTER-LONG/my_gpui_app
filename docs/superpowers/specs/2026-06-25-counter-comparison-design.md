# 计数器对比测试组件 — 设计文档

## 目标

用 GPUI 原生（div-based）和 gpui-component 两种方式分别实现计数器 UI，并排放置在同一窗口中，直观对比两种开发方式的差异。

## 架构

```
MainView (h_flex 左右分栏)
  ├── NativeCounterView (左侧，原生 GPUI div)
  └── ComponentCounterView (右侧，gpui-component Button + Label)
```

两边各自拥有独立的 `count` / `fps` 状态，互不干扰。

## 文件结构

```
src/
├── main.rs              # 入口：gpui_component::init + Root + MainView
├── native_counter.rs    # NativeCounterView：纯 div 实现
└── component_counter.rs # ComponentCounterView：gpui-component 实现
```

## 各组件设计

### main.rs — MainView

- 初始化 `gpui_component::init(cx)`
- 用 `Root` 包裹 MainView
- MainView 包含两个子视图 `NativeCounterView` 和 `ComponentCounterView`
- 布局：`h_flex().size_full().gap_4().p_4()`，左右各 `flex_1()`
- 每侧用 `v_flex().border().rounded_md().p_4().gap_2()` 包裹

### native_counter.rs — NativeCounterView

- 字段：`count: i32`, `fps: f64`, `last_frame: Instant`, `frame_count: u64`, `accumulated_time: f64`
- 使用现有 FPS 计算逻辑
- 标题：`div().text_xl().font_weight(BOLD)`
- 计数：`div().text_3xl().font_weight(BOLD)`
- 按钮：`div().px_4().py_2().bg().rounded_md().cursor_pointer().on_click()`
- 颜色：硬编码 `rgb(0x...)`

### component_counter.rs — ComponentCounterView

- 字段：同上
- 标题：`Label::new(...).text_xl()`
- 计数：`Label::new(...).text_3xl()`
- 按钮：`Button::new("incr").primary().label("Increment").on_click()`
- 颜色：使用 `cx.theme().primary` 等主题系统
- FPS：用 `Label` 显示

## 关键差异对比

| 方面 | 原生 GPUI | gpui-component |
|------|----------|----------------|
| 按钮创建 | `div()` + `.on_click()` | `Button::new("id").primary()` |
| 文本颜色 | `rgb(0xffffff)` 硬编码 | `cx.theme().foreground` |
| 背景色 | `rgb(0x2e2e2e)` 硬编码 | `cx.theme().background` |
| 按钮颜色 | `rgb(0x007acc)` 硬编码 | `Button::primary()` 自动适配主题 |
| 圆角 | `.rounded_md()` 手动 | Button 自带圆角 |
| 交互反馈 | 仅 cursor_pointer | Button 自带 hover/active 效果 |
| 显示文本 | `div().child("text")` | `Label::new("id").child("text")` |

## 初始化

```rust
// main.rs
fn main() {
    application().run(|cx: &mut App| {
        gpui_component::init(cx);  // 必须最先调用

        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|_| MainView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx));
        }).unwrap();
        cx.activate(true);
    });
}
```

## 验证

- `cargo check` 编译通过
- `cargo run` 运行，左右两个计数器独立工作
- 按钮点击后计数正确 update
- FPS 正常显示
