use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;
use gpui_kit::Size;

pub struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx| {
        // this must be called before using any GPUI Component features.
        gpui_kit::init(cx);

        let mut options = WindowOptions::default();
        options.window_bounds = Some(WindowBounds::Windowed(
            Bounds::centered(None, Size { width: Pixels::from(300.0), height: Pixels::from(200.0) }, cx)
        ));

        cx.spawn(async move |cx| {
            let window = cx.open_window(options, |window, cx| {
                let view = cx.new(|_| HelloWorld);
                // this first level on the window, should be a Root.
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");

        }).detach();
    });

}
