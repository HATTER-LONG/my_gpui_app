use gpui::{prelude::*, rgb, Context, Window, div};
use std::time::Instant;

pub struct NativeCounterView {
    count: i32,
    fps: f64,
    last_frame: Instant,
    frame_count: u64,
    accumulated_time: f64,
}

impl NativeCounterView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            count: 0,
            fps: 0.0,
            last_frame: Instant::now(),
            frame_count: 0,
            accumulated_time: 0.0,
        }
    }
}

impl Render for NativeCounterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        self.frame_count += 1;

        self.accumulated_time += delta;
        if self.accumulated_time >= 0.5 {
            self.fps = self.frame_count as f64 / self.accumulated_time;
            self.frame_count = 0;
            self.accumulated_time = 0.0;
        }

        window.request_animation_frame();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_2()
            .p_4()
            .border_1()
            .border_color(rgb(0x555555))
            .rounded_md()
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Native GPUI"),
            )
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(format!("Count: {}", self.count)),
            )
            .child(
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
                            .id("native-incr")
                            .px_4()
                            .py_2()
                            .bg(rgb(0x007acc))
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .child("Increment")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("native-decr")
                            .px_4()
                            .py_2()
                            .bg(rgb(0xcc3333))
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .child("Decrement")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}
