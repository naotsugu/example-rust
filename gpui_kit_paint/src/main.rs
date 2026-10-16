#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;
use gpui_kit::*;

// palette (column-major pairs: top row, bottom row)
const PALETTE: [u32; 20] = [
    0x000000, 0xffffff, // black, white
    0x7f7f7f, 0xc3c3c3, // dark gray, light gray
    0x880015, 0xb97a57, // dark red, brown
    0xed1c24, 0xffaec9, // red, rose
    0xff7f27, 0xffc90e, // orange, gold
    0xfff200, 0xefe4b0, // yellow, light yellow
    0x22b14c, 0xb5e61d, // green, lime
    0x00a2e8, 0x99d9ea, // turquoise, light turquoise
    0x3f48cc, 0x7092be, // indigo, blue-gray
    0xa349a4, 0xc8bfe7, // purple, lavender
];

const SIDEBAR_WIDTH: f32 = 46.;
const SWATCH_SIZE: f32 = 16.;

pub struct Application {
    strokes: Rc<Vec<Stroke>>,
    drawing: bool,
    current_color: Hsla,
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
            drawing: false,
            current_color: rgb(PALETTE[0]).into(),
        }
    }
    fn handle_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drawing = true;
        // start a new stroke at the pressed position
        Rc::make_mut(&mut self.strokes).push(Stroke {
            points: vec![event.position],
            color: self.current_color,
            size: 3.0,
        });
        cx.notify();
    }

    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drawing {
            if let Some(stroke) = Rc::make_mut(&mut self.strokes).last_mut() {
                stroke.points.push(event.position);
            }
        }
        cx.notify();
    }

    fn handle_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drawing = false;
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // current color indicator (like "Color 1" in Paint)
        let current = div()
            .w(px(SWATCH_SIZE * 2. + 4.))
            .h(px(SWATCH_SIZE * 2.))
            .bg(self.current_color)
            .border_1()
            .border_color(rgb(0x808080));

        // Palette grid: 2 columns, wrapped
        let mut grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(4.))
            .w(px(SWATCH_SIZE * 2. + 4.));

        for hex in PALETTE {
            let color: Hsla = rgb(hex).into();
            let selected = self.current_color == color;
            grid = grid.child(
                div()
                    .w(px(SWATCH_SIZE))
                    .h(px(SWATCH_SIZE))
                    .bg(color)
                    .border_2()
                    .border_color(if selected { rgb(0x0078d7) } else { rgb(0x808080) })
                    .cursor(CursorStyle::PointingHand)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            // Change the drawing color for subsequent strokes
                            this.current_color = color;
                            cx.notify();
                        }),
                    ),
            );
        }

        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .p(px(10.))
            .bg(rgb(0xf0f0f0))
            .border_r_1()
            .border_color(rgb(0xd0d0d0))
            .child(current)
            .child(grid)
    }
}

impl Render for Application {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {

        let strokes = self.strokes.clone();
        let sidebar = self.render_sidebar(cx);

        div()
            .size_full()
            .flex()
            .flex_row()
            .child(sidebar)
            .child(
                // Drawing area: mouse handlers live here so sidebar clicks don't draw
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .bg(rgb(0xffffff))
                    .cursor(CursorStyle::Crosshair)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
                    .on_mouse_move(cx.listener(Self::handle_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::handle_mouse_up))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |_, _, window, _| {
                                // Draw strokes
                                for stroke in strokes.iter() {
                                    // A single point cannot form a line
                                    if stroke.points.len() < 2 {
                                        continue;
                                    }
                                    let mut builder = PathBuilder::stroke(px(stroke.size));
                                    builder.move_to(stroke.points[0]);
                                    for p in &stroke.points[1..] {
                                        builder.line_to(*p);
                                    }
                                    if let Ok(path) = builder.build() {
                                        window.paint_path(path, stroke.color);
                                    }
                                }
                            },
                        )
                            .size_full(),
                    ),
            )
    }
}

fn main() {

    application().run(move |cx| {

        gpui_kit::init(cx);

        let window_options = window_options(cx);
        open_window(window_options, cx, |_, cx| cx.new(|_| Application::new()))
            .expect("failed to open window");

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        }).detach();

        cx.activate(true);

    });
}

fn window_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
}