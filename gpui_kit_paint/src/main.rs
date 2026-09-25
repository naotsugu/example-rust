#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;
use gpui_kit::component::*;
use gpui_kit::*;

pub struct Application {
    strokes: Rc<Vec<Stroke>>,
}

#[derive(Clone, Debug)]
struct Stroke {
    points: Vec<Point<Pixels>>,
    color: Hsla,
    size: f32,
}

impl Application {
    fn new() -> Self {
        Self {
            strokes: Rc::new(vec![]),
        }
    }
    fn handle_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
    }
    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
    }
    fn handle_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
    }
}

impl Render for Application {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("Hello, World!")
    }
}

fn main() {

    application().run(move |cx| {

        gpui_kit::init(cx);

        let window_options = window_options(cx);

        cx.open_window(window_options, |window, cx| {
            let app = Application::new();
            let view = cx.new(|_| app);
            cx.new(|cx| Root::new(view, window, cx))
        }).expect("failed to open window");

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        }).detach();

        cx.activate(true);

    });
}

fn window_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(300.), px(200.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
}