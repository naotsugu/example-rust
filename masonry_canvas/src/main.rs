
#![cfg_attr(not(test), windows_subsystem = "windows")]

use masonry::core::{ErasedAction, NewWidget, Widget as _, WidgetId};
use masonry::dpi::LogicalSize;
use masonry::imaging::Painter;
use masonry::kurbo::{Affine, BezPath, Stroke};
use masonry::peniko::Color;
use masonry::theme::default_property_set;
use masonry::widgets::{Canvas, Flex};
use masonry_winit::app::{AppDriver, DriverCtx, NewWindow, WindowId};
use masonry_winit::winit::window::Window;

struct Driver {
    window_id: WindowId,
}

impl AppDriver for Driver {
    fn on_action(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_, '_>,
        widget_id: WidgetId,
        action: ErasedAction,
    ) {

    }
}

fn widget_tree() -> NewWidget<impl masonry::core::Widget> {
    let canvas = Canvas::default();
    NewWidget::new(
        Flex::column()
            .with_fixed(canvas.prepare()),
    )
}


fn main() {

    let window_size = LogicalSize::new(300.0, 200.0);
    let window_attributes = Window::default_attributes()
        .with_title("masonry")
        .with_resizable(true)
        .with_inner_size(window_size);

    let driver = Driver {
        window_id: WindowId::next(),
    };

    masonry_winit::app::run(
        vec![NewWindow::new_with_id(
            driver.window_id,
            window_attributes,
            widget_tree().erased(),
        )],
        driver,
        default_property_set(),
    )
    .unwrap();
}
