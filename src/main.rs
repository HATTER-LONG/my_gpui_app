use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use std::time::Instant;

struct MyWindow {
    count: i32,
    fps: f64,
    last_frame: Instant,
    frame_count: u64,
    accumulated_time: f64,
}

impl Render for MyWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // -------- 帧率计算 --------
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        self.frame_count += 1;

        // 每 0.5 秒更新一次 FPS 显示值（避免显示值抖动过快）
        self.accumulated_time += delta;
        if self.accumulated_time >= 0.5 {
            self.fps = self.frame_count as f64 / self.accumulated_time;
            self.frame_count = 0;
            self.accumulated_time = 0.0;
        }

        // 请求下一帧回调——让 GPUI 持续渲染（0 参数，无 cx）
        window.request_animation_frame();

        // -------- UI --------
        div()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(0x2e2e2e))
            .size_full()
            .justify_center()
            .items_center()
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(format!("Count: {}", self.count)),
            )
            .child(
                // FPS 显示
                div()
                    .text_sm()
                    .text_color(rgb(0x888888))
                    .child(format!("FPS: {:.0}", self.fps)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("increment")
                            .px_4()
                            .py_2()
                            .bg(rgb(0x007acc))
                            .rounded_md()
                            .cursor_pointer()
                            .child("Increment")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("decrement")
                            .px_4()
                            .py_2()
                            .bg(rgb(0xcc3333))
                            .rounded_md()
                            .cursor_pointer()
                            .child("Decrement")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(400.0), px(300.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| MyWindow {
                    count: 0,
                    fps: 0.0,
                    last_frame: Instant::now(),
                    frame_count: 0,
                    accumulated_time: 0.0,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
