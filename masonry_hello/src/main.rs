use masonry::core::Widget;

struct Driver {
    window_id: masonry_winit::app::WindowId,
}

impl masonry_winit::app::AppDriver for Driver {
    fn on_action(
        &mut self,
        window_id: masonry_winit::app::WindowId,
        _ctx: &mut masonry_winit::app::DriverCtx<'_, '_>,
        _widget_id: masonry::core::WidgetId,
        action: masonry::core::ErasedAction,
    ) {
        debug_assert_eq!(window_id, self.window_id, "unknown window");

        if action.is::<masonry::widgets::ButtonPress>() {
            println!("Hello");
        } else {
            eprintln!("Unexpected action {action:?}");
        }
    }
}

fn main() {

    let label = masonry::widgets::Label::new("Hello World!");
    let button = masonry::widgets::Button::with_text("Say hello");

    let main_widget = masonry::widgets::Flex::column()
        .with_child(label.with_auto_id())
        .with_child(button.with_auto_id());

    let driver = Driver {
        window_id: masonry_winit::app::WindowId::next(),
    };

    let window_attributes = masonry_winit::winit::window::Window::default_attributes()
        .with_title("masonry")
        .with_resizable(true)
        .with_inner_size(masonry::dpi::LogicalSize::new(300.0, 200.0));

    masonry_winit::app::run(
        masonry_winit::app::EventLoop::with_user_event(),
        vec![masonry_winit::app::NewWindow::new_with_id(
            driver.window_id,
            window_attributes,
            masonry::core::NewWidget::new(main_widget).erased(),
        )],
        driver,
        masonry::theme::default_property_set(),
    )
    .unwrap();
}
