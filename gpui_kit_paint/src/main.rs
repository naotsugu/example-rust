#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;
use gpui_kit::*;
use gpui_kit::assets::Assets;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::{ActiveTheme, Sizable};

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

const SIDEBAR_WIDTH: f32 = 96.;
const SWATCH_SIZE: f32 = 16.;

pub struct Application {
    strokes: Rc<Vec<Stroke>>,
    drawing: bool,
    brush_size: f32,
    current_color: Hsla,
    size_input: Entity<InputState>,
    color_picker: Entity<ColorPickerState>,
    _subscriptions: Vec<Subscription>, // keep subscriptions alive for the lifetime of the view
}

#[derive(Clone, Debug)]
struct Stroke {
    points: Vec<Point<Pixels>>,
    color: Hsla,
    size: f32,
}

impl Application {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {

        // brush size input
        let size_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("3")
                .step(1.)
                .min(1.)
        });

        // reflect the number input into the brush size
        let size_sub = cx.subscribe_in(&size_input, window, |this, state, ev, _window, cx| {
            if let InputEvent::Change = ev {
                let text = state.read(cx).value();
                // a typed value may be empty or out of range while editing; ignore/clamp it
                if let Ok(v) = text.parse::<f32>() {
                    if v.is_finite() {
                        this.brush_size = v.clamp(1., 500.);
                        cx.notify();
                    }
                }
            }
        });

        let initial_color: Hsla = rgb(PALETTE[0]).into();
        let color_picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(initial_color));

        // reflect colors chosen in the picker popover into the drawing color
        let color_sub = cx.subscribe(&color_picker, |this, _, ev, cx| match ev {
            ColorPickerEvent::Change(color) => {
                if let Some(color) = color {
                    this.current_color = *color;
                    cx.notify();
                }
            }
        });
        Self {
            strokes: Rc::new(vec![]),
            drawing: false,
            brush_size: 3.,
            current_color: initial_color,
            size_input,
            color_picker,
            _subscriptions: vec![color_sub, size_sub],
        }
    }
    fn handle_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drawing = true;
        // start a new stroke at the pressed position
        Rc::make_mut(&mut self.strokes).push(Stroke {
            points: vec![event.position],
            color: self.current_color,
            size: self.brush_size,
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

        // brush size input
        let brush_size = div()
            .w_full()
            .child(NumberInput::new(&self.size_input).xsmall());

        // current color indicator
        let current = ColorPicker::new(&self.color_picker).anchor(Anchor::TopRight);

        // palette grid: 2 columns, wrapped
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
                        cx.listener(move |this, _, window, cx| {
                            // change the drawing color for subsequent strokes
                            this.current_color = color;
                            // keep the picker's displayed color in sync (does not emit Change)
                            this.color_picker
                                .update(cx, |state, cx| state.set_value(color, window, cx));
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
            .bg(cx.theme().secondary)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(brush_size)
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
                // drawing area: mouse handlers live here so sidebar clicks don't draw
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
                                // draw strokes
                                for stroke in strokes.iter() {
                                    // a single point cannot form a line
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

    application().with_assets(Assets).run(move |cx| {

        gpui_kit::init(cx);

        let window_options = window_options(cx);
        open_window(window_options, cx, |window, cx| cx.new(|cx| Application::new(window, cx)))
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