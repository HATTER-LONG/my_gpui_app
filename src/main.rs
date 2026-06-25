mod native_counter;
mod component_counter;

use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::{ActiveTheme as _, Root};
use gpui_platform::application;
use native_counter::NativeCounterView;
use component_counter::ComponentCounterView;

struct MainView {
    native: gpui::Entity<NativeCounterView>,
    component: gpui::Entity<ComponentCounterView>,
}

impl MainView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            native: cx.new(|cx| NativeCounterView::new(window, cx)),
            component: cx.new(|cx| ComponentCounterView::new(window, cx)),
        }
    }
}

impl gpui::Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .gap_4()
            .p_4()
            .bg(cx.theme().background)
            .child(self.native.clone())
            .child(self.component.clone())
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(640.0), px(360.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| MainView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
