#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;
use gpui_kit::*;

pub struct Application {
    strokes: Rc<Vec<Stroke>>,
    drawing: bool,
    hover: Option<Point<Pixels>>,
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
            hover: None,
        }
    }
    fn handle_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drawing = true;
        // Start a new stroke at the pressed position
        Rc::make_mut(&mut self.strokes).push(Stroke {
            points: vec![event.position],
            color: hsla(0., 0., 0.1, 1.),
            size: 3.0,
        });
        cx.notify();
    }

    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.hover = Some(event.position);
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
}

impl Render for Application {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strokes = self.strokes.clone();
        let hover = self.hover;

        div()
            .size_full()
            .bg(rgb(0xffffff))
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            // .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
            //     if !*hovered {
            //         this.hover = None;
            //         cx.notify();
            //     }
            // }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        // draw strokes
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

                        // // draw the pen tip as a circle outline (brush size = 3.0 here)
                        // if let Some(center) = hover {
                        //     let radius = 3.0_f32 / 2.0 + 2.0;
                        //     let cx_ = f32::from(center.x);
                        //     let cy_ = f32::from(center.y);
                        //     let segments = 24;
                        //
                        //     let mut builder = PathBuilder::stroke(px(1.0));
                        //     for i in 0..=segments {
                        //         let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                        //         let p = point(px(cx_ + radius * t.cos()), px(cy_ + radius * t.sin()));
                        //         if i == 0 {
                        //             builder.move_to(p);
                        //         } else {
                        //             builder.line_to(p);
                        //         }
                        //     }
                        //     if let Ok(path) = builder.build() {
                        //         window.paint_path(path, hsla(0., 0., 0.4, 1.));
                        //     }
                        // }

                    },
                )
                .size_full(),
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
    let bounds = Bounds::centered(None, size(px(300.), px(200.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
}