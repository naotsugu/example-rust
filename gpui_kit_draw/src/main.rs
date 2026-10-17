#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::Cell;
use std::rc::Rc;
use gpui_kit::*;
use gpui_kit::assets::Assets;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
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
const FOOTER_HEIGHT: f32 = 32.;

// minimum gap between the canvas and the viewport edge when scrolled to an end
const CANVAS_MARGIN: f32 = 12.;

pub struct Application {
    canvas_width: f32,
    canvas_height: f32,
    // stroke points are stored in canvas coordinates (independent of zoom and scroll)
    strokes: Rc<Vec<Stroke>>,
    drawing: bool,
    brush_size: f32,
    current_color: Hsla,
    zoom_percent: f32,
    size_input: Entity<InputState>,
    color_picker: Entity<ColorPickerState>,
    zoom_slider: Entity<SliderState>,
    // scroll state of the viewport, used to keep the zoom centered
    scroll_handle: ScrollHandle,
    // size of the viewport area (window minus sidebar and footer), updated every render
    viewport_size: Size<Pixels>,
    // canvas bounds in window coordinates, recorded every frame during prepaint
    canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
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

        // zoom slider (percent)
        let zoom_slider = cx.new(|_| {
            SliderState::new()
                .min(10.)
                .max(400.)
                .step(5.)
                .default_value(100.)
        });

        // reflect the slider into the zoom factor, keeping the viewport center fixed
        let zoom_sub = cx.subscribe(&zoom_slider, |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(value) = ev {
                this.apply_zoom(value.start());
                cx.notify();
            }
        });

        Self {
            canvas_width: 600.,
            canvas_height: 500.,
            strokes: Rc::new(vec![]),
            drawing: false,
            brush_size: 3.,
            current_color: initial_color,
            zoom_percent: 100.,
            size_input,
            color_picker,
            zoom_slider,
            scroll_handle: ScrollHandle::new(),
            viewport_size: Size::default(),
            canvas_bounds: Rc::new(Cell::new(Bounds::default())),
            _subscriptions: vec![color_sub, size_sub, zoom_sub],
        }
    }

    // zoom factor (1.0 = 100%)
    fn zoom(&self) -> f32 {
        self.zoom_percent / 100.
    }

    // size of the scrollable content at the given zoom: the canvas plus margins, but never
    // smaller than the viewport (so the canvas can be centered when it is small)
    fn content_size(&self, zoom: f32) -> (f32, f32) {
        let view_w = f32::from(self.viewport_size.width);
        let view_h = f32::from(self.viewport_size.height);
        (
            (self.canvas_width * zoom + CANVAS_MARGIN * 2.).max(view_w),
            (self.canvas_height * zoom + CANVAS_MARGIN * 2.).max(view_h),
        )
    }

    // change the zoom while keeping the canvas point at the viewport center in place
    fn apply_zoom(&mut self, new_percent: f32) {
        let old_zoom = self.zoom();
        let new_zoom = new_percent / 100.;

        let view_w = f32::from(self.viewport_size.width);
        let view_h = f32::from(self.viewport_size.height);
        let (old_w, old_h) = self.content_size(old_zoom);
        let (new_w, new_h) = self.content_size(new_zoom);

        // the scroll offset is <= 0 (content moves left/up as the viewport scrolls right/down)
        let offset = self.scroll_handle.offset();

        // offset of the viewport center from the canvas center, in canvas units
        // (the canvas is centered inside the content, so the canvas center is the content center)
        let dx = (-f32::from(offset.x) + view_w / 2. - old_w / 2.) / old_zoom;
        let dy = (-f32::from(offset.y) + view_h / 2. - old_h / 2.) / old_zoom;

        // new offset that puts the same canvas point back at the viewport center
        let new_x = (view_w / 2. - (new_w / 2. + dx * new_zoom)).clamp(-(new_w - view_w).max(0.), 0.);
        let new_y = (view_h / 2. - (new_h / 2. + dy * new_zoom)).clamp(-(new_h - view_h).max(0.), 0.);

        self.scroll_handle.set_offset(point(px(new_x), px(new_y)));
        self.zoom_percent = new_percent;
    }


    // convert a window position into canvas coordinates
    fn to_canvas(&self, pos: Point<Pixels>) -> Point<Pixels> {
        let origin = self.canvas_bounds.get().origin;
        let zoom = self.zoom();
        point((pos.x - origin.x) / zoom, (pos.y - origin.y) / zoom)
    }

    fn handle_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drawing = true;
        // start a new stroke at the pressed position
        let start = self.to_canvas(event.position);
        Rc::make_mut(&mut self.strokes).push(Stroke {
            points: vec![start],
            color: self.current_color,
            size: self.brush_size,
        });
        cx.notify();
    }

    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drawing {
            let p = self.to_canvas(event.position);
            if let Some(stroke) = Rc::make_mut(&mut self.strokes).last_mut() {
                stroke.points.push(p);
            }
            cx.notify();
        }
    }

    fn handle_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drawing = false;
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {

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

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .w_full()
            .h(px(FOOTER_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .px(px(12.))
            .bg(cx.theme().secondary)
            .border_t_1()
            .border_color(cx.theme().sidebar_border)
            .child(div().text_xs().child("Zoom"))
            .child(Slider::new(&self.zoom_slider).w(px(160.)))
            .child(
                div()
                    .w(px(44.))
                    .text_xs()
                    .child(format!("{}%", self.zoom_percent.round() as i32)),
            )
    }
}

impl Render for Application {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {

        // the viewport is whatever remains of the window after the fixed sidebar and footer
        let window_size = window.viewport_size();
        self.viewport_size = size(
            window_size.width - px(SIDEBAR_WIDTH),
            window_size.height - px(FOOTER_HEIGHT),
        );

        let strokes = self.strokes.clone();
        let zoom = self.zoom();
        let canvas_bounds = self.canvas_bounds.clone();
        let (content_w, content_h) = self.content_size(zoom);
        let sidebar = self.render_sidebar(cx);
        let footer = self.render_footer(cx);


        // the canvas (document): fixed size scaled by the zoom factor
        let document = div()
            .relative()
            .flex_none()
            .w(px(self.canvas_width * zoom))
            .h(px(self.canvas_height * zoom))
            .overflow_hidden()
            .bg(rgb(0xffffff))
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .child(
                canvas(
                    // record the canvas bounds (window coordinates, scroll offset included)
                    move |bounds, _, _| canvas_bounds.set(bounds),
                    move |bounds, _, window, _| {
                        // draw strokes: canvas coordinates -> window coordinates
                        for stroke in strokes.iter() {
                            // a single point cannot form a line
                            if stroke.points.len() < 2 {
                                continue;
                            }
                            let to_window = |p: Point<Pixels>| {
                                point(bounds.origin.x + p.x * zoom, bounds.origin.y + p.y * zoom)
                            };
                            let mut builder = PathBuilder::stroke(px(stroke.size * zoom));
                            builder.move_to(to_window(stroke.points[0]));
                            for p in &stroke.points[1..] {
                                builder.line_to(to_window(*p));
                            }
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, stroke.color);
                            }
                        }
                    },
                )
                    .size_full(),
            );

        // scrollable content: at least as large as the viewport, with the canvas centered in it
        let content = div()
            .flex_none()
            .w(px(content_w))
            .h(px(content_h))
            .flex()
            .items_center()
            .justify_center()
            .child(document);

        // viewport: scrollable area holding the canvas; move/up handlers live here so a
        // stroke keeps following the mouse even when it leaves the canvas
        let viewport = div()
            .id("viewport")
            .flex_1()
            .min_w_0()
            .h_full()
            .track_scroll(&self.scroll_handle)
            .overflow_scroll()
            .bg(rgb(0xc8c8c8))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .child(content);

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(sidebar)
                    .child(viewport),
            )
            .child(footer)
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