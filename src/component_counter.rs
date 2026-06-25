use gpui::{prelude::*, Context, Window, div};
use gpui_component::button::Button;
use gpui_component::label::Label;
use gpui_component::ActiveTheme as _;
use std::time::Instant;

pub struct ComponentCounterView {
    count: i32,
    fps: f64,
    last_frame: Instant,
    frame_count: u64,
    accumulated_time: f64,
}

impl ComponentCounterView {
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

impl Render for ComponentCounterView {
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
            .border_color(cx.theme().border)
            .rounded_md()
            .bg(cx.theme().background)
            .child(
                Label::new("comp-title")
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("Component"),
            )
            .child(
                Label::new("comp-count")
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(format!("Count: {}", self.count)),
            )
            .child(
                Label::new("comp-fps")
                    .text_sm()
                    .text_color(cx.theme().muted)
                    .child(format!("FPS: {:.0}", self.fps)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("comp-incr")
                            .primary()
                            .label("Increment")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("comp-decr")
                            .danger()
                            .label("Decrement")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}
