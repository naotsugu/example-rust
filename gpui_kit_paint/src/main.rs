#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use gpui_kit::*;
use gpui_kit::assets::{AllAssets, IconName};
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle};
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogFooter;

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

const SIDEBAR_WIDTH: f32 = 112.;
const SWATCH_SIZE: f32 = 16.;
const BUTTON_WIDTH: f32 = 44.;
const FOOTER_HEIGHT: f32 = 36.;

// minimum gap between the canvas and the viewport edge when scrolled to an end
const CANVAS_MARGIN: f32 = 12.;

const MIN_ZOOM_PERCENT: f32 = 25.;
const MAX_ZOOM_PERCENT: f32 = 1600.;
// the pixel grid is shown from this zoom factor (8.0 = 800%)
const GRID_MIN_ZOOM: f32 = 8.;

const MAX_CANVAS_SIZE: usize = 4096;
const MAX_HISTORY: usize = 100;

const BACKGROUND: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
// color of the area around the canvas
const SURROUND: [u8; 3] = [0xc8, 0xc8, 0xc8];

/// An opaque RGBA bitmap: the source of truth of the document.
/// It tracks the bounding box of modified pixels so that history entries stay small.
#[derive(Clone)]
struct Bitmap {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>,
    // modified area since the last history entry: (x0, y0, x1, y1), end-exclusive
    dirty: Option<(i32, i32, i32, i32)>,
}

impl Bitmap {
    fn new(width: usize, height: usize, fill: [u8; 4]) -> Self {
        Self { width, height, pixels: vec![fill; width * height], dirty: None }
    }

    // grow the dirty area; the given rectangle must already be clipped to the bitmap
    fn mark(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        self.dirty = Some(match self.dirty {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
    }

    fn get(&self, x: i32, y: i32) -> Option<[u8; 4]> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(self.pixels[y as usize * self.width + x as usize])
    }

    fn set(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        self.pixels[y as usize * self.width + x as usize] = color;
        self.mark(x, y, x + 1, y + 1);
    }

    /// Fill a rectangle, clipped to the bitmap.
    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: [u8; 4]) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            let row = yy as usize * self.width;
            self.pixels[row + x0 as usize..row + x1 as usize].fill(color);
        }
        self.mark(x0, y0, x1, y1);
    }

    /// Paint a square brush of `size` pixels centered on (cx, cy).
    fn stamp(&mut self, cx: i32, cy: i32, size: i32, color: [u8; 4]) {
        self.fill_rect(cx - size / 2, cy - size / 2, size, size, color);
    }

    /// Draw a line with Bresenham's algorithm, stamping the brush at every step.
    fn line(&mut self, from: (i32, i32), to: (i32, i32), size: i32, color: [u8; 4]) {
        let (mut x0, mut y0) = from;
        let (x1, y1) = to;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.stamp(x0, y0, size, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Rectangle between two inclusive corners.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, size: i32, filled: bool, color: [u8; 4]) {
        if filled {
            self.fill_rect(x0, y0, x1 - x0 + 1, y1 - y0 + 1, color);
            return;
        }
        self.line((x0, y0), (x1, y0), size, color);
        self.line((x1, y0), (x1, y1), size, color);
        self.line((x1, y1), (x0, y1), size, color);
        self.line((x0, y1), (x0, y0), size, color);
    }

    /// Ellipse inscribed in the rectangle between two inclusive corners.
    fn ellipse(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, size: i32, filled: bool, color: [u8; 4]) {
        let cx = (x0 + x1) as f32 / 2.;
        let cy = (y0 + y1) as f32 / 2.;
        let a = (x1 - x0) as f32 / 2.;
        let b = (y1 - y0) as f32 / 2.;

        if filled {
            // one span per row
            for y in y0..=y1 {
                let t = if b > 0. { (y as f32 - cy) / b } else { 0. };
                let half = a * (1. - t * t).max(0.).sqrt();
                let left = (cx - half).round() as i32;
                let right = (cx + half).round() as i32;
                self.fill_rect(left, y, right - left + 1, 1, color);
            }
            return;
        }

        // sample the outline densely and connect the samples with lines, so no gaps remain
        let steps = ((a.max(b) * 4.) as usize).max(16);
        let mut prev: Option<(i32, i32)> = None;
        for i in 0..=steps {
            let theta = i as f32 / steps as f32 * std::f32::consts::TAU;
            let p = ((cx + a * theta.cos()).round() as i32, (cy + b * theta.sin()).round() as i32);
            match prev {
                Some(q) if q != p => self.line(q, p, size, color),
                None => self.stamp(p.0, p.1, size, color),
                _ => {}
            }
            prev = Some(p);
        }
    }

    /// Scanline flood fill (4-connected) that replaces the region of the clicked color.
    fn flood_fill(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let Some(target) = self.get(x, y) else { return };
        if target == color {
            return;
        }
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            if self.get(x, y) != Some(target) {
                continue;
            }
            // extend the span to the left and right
            let mut left = x;
            while self.get(left - 1, y) == Some(target) {
                left -= 1;
            }
            let mut right = x;
            while self.get(right + 1, y) == Some(target) {
                right += 1;
            }
            for i in left..=right {
                self.set(i, y, color);
            }
            // queue one seed per run of target pixels in the rows above and below
            for ny in [y - 1, y + 1] {
                let mut inside = false;
                for i in left..=right {
                    let matches = self.get(i, ny) == Some(target);
                    if matches && !inside {
                        stack.push((i, ny));
                    }
                    inside = matches;
                }
            }
        }
    }

    /// Copy a rectangle into a new bitmap (pixels outside of this bitmap become background).
    fn copy_region(&self, x: i32, y: i32, w: i32, h: i32) -> Bitmap {
        let mut out = Bitmap::new(w.max(1) as usize, h.max(1) as usize, BACKGROUND);
        for yy in 0..h {
            for xx in 0..w {
                if let Some(p) = self.get(x + xx, y + yy) {
                    out.pixels[yy as usize * out.width + xx as usize] = p;
                }
            }
        }
        out
    }

    /// Overwrite a part of this bitmap with `src` placed at (x, y), clipped.
    fn paste(&mut self, src: &Bitmap, x: i32, y: i32) {
        for sy in 0..src.height as i32 {
            let dy = y + sy;
            if dy < 0 || dy >= self.height as i32 {
                continue;
            }
            for sx in 0..src.width as i32 {
                let dx = x + sx;
                if dx < 0 || dx >= self.width as i32 {
                    continue;
                }
                self.pixels[dy as usize * self.width + dx as usize] =
                    src.pixels[sy as usize * src.width + sx as usize];
            }
        }
        self.mark(
            x.max(0),
            y.max(0),
            (x + src.width as i32).min(self.width as i32),
            (y + src.height as i32).min(self.height as i32),
        );
    }

    /// Restore a rectangle from `src` (a bitmap of the same size), clipped.
    fn restore_region(&mut self, src: &Bitmap, rect: (i32, i32, i32, i32)) {
        let x0 = rect.0.max(0);
        let y0 = rect.1.max(0);
        let x1 = rect.2.min(self.width as i32);
        let y1 = rect.3.min(self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let row = y as usize * self.width;
            let range = row + x0 as usize..row + x1 as usize;
            self.pixels[range.clone()].copy_from_slice(&src.pixels[range]);
        }
        self.mark(x0, y0, x1, y1);
    }

    // history helpers: these do not touch the dirty area
    fn read_region(&self, x: usize, y: usize, w: usize, h: usize) -> Vec<[u8; 4]> {
        let mut out = Vec::with_capacity(w * h);
        for yy in y..y + h {
            let row = yy * self.width;
            out.extend_from_slice(&self.pixels[row + x..row + x + w]);
        }
        out
    }

    fn write_region(&mut self, x: usize, y: usize, w: usize, h: usize, data: &[[u8; 4]]) {
        for (i, yy) in (y..y + h).enumerate() {
            let row = yy * self.width;
            self.pixels[row + x..row + x + w].copy_from_slice(&data[i * w..(i + 1) * w]);
        }
    }
}

/// One undo step. A patch stores only the changed rectangle.
enum Edit {
    Patch {
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        before: Vec<[u8; 4]>,
        after: Vec<[u8; 4]>,
    },
    // used when the canvas size changes
    Replace {
        before: Bitmap,
        after: Bitmap,
    },
}

/// A rectangular selection. The pixels are lifted ("floating") when the selection is first moved.
struct Selection {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    floating: Option<Bitmap>,
}

impl Selection {
    fn contains(&self, p: (i32, i32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pencil,
    Eraser,
    Fill,
    Picker,
    Line,
    Rect,
    Oval,
    Select,
}

/// What the current mouse drag is doing.
#[derive(Clone, Copy)]
enum Drag {
    None,
    Draw,
    Shape { start: (i32, i32) },
    SelectNew { start: (i32, i32) },
    SelectMove { grab: (i32, i32) },
}

pub struct Application {
    bitmap: Bitmap,
    // the bitmap as of the last history entry, used to build patches
    committed: Bitmap,
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
    tool: Tool,
    drag: Drag,
    // last pixel touched while dragging (canvas coordinates)
    last_pos: (i32, i32),
    brush_size: i32,
    shape_filled: bool,
    // shape preview: the bitmap before the shape and the area the preview covers
    shape_backup: Option<Bitmap>,
    shape_bounds: Option<(i32, i32, i32, i32)>,
    selection: Option<Selection>,
    // pixel under the mouse pointer, if it is inside the canvas
    cursor_px: Option<(i32, i32)>,
    current_color: Hsla,
    zoom_percent: f32,
    // how far the viewport is scrolled into the content, in logical pixels (>= 0)
    scroll: (f32, f32),
    size_input: Entity<InputState>,
    width_input: Entity<InputState>,
    height_input: Entity<InputState>,
    color_picker: Entity<ColorPickerState>,
    zoom_slider: Entity<SliderState>,
    // size of the viewport area (window minus sidebar and footer), updated every render
    viewport_size: Size<Pixels>,
    // viewport origin in window coordinates, recorded every frame during prepaint
    viewport_origin: Rc<Cell<Point<Pixels>>>,
    // the visible part of the bitmap, already magnified (device pixel resolution)
    view_image: Option<Arc<RenderImage>>,
    view_pixels: (usize, usize),
    view_dirty: bool,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>, // keep subscriptions alive for the lifetime of the view
}

impl Application {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {

        // brush size input
        let size_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("1")
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
                        this.brush_size = (v.round() as i32).clamp(1, 500);
                        cx.notify();
                    }
                }
            }
        });

        // canvas size inputs (applied with the Resize button)
        let width_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("600")
                .step(10.)
                .min(1.)
        });
        let height_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("500")
                .step(10.)
                .min(1.)
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
                .min(MIN_ZOOM_PERCENT)
                .max(MAX_ZOOM_PERCENT)
                .step(25.)
                .default_value(100.)
        });

        // reflect the slider into the zoom factor, keeping the viewport center fixed
        let zoom_sub = cx.subscribe(&zoom_slider, |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(value) = ev {
                this.apply_zoom(value.start());
                cx.notify();
            }
        });

        let bitmap = Bitmap::new(600, 500, BACKGROUND);

        Self {
            committed: bitmap.clone(),
            bitmap,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            tool: Tool::Pencil,
            drag: Drag::None,
            last_pos: (0, 0),
            brush_size: 1,
            shape_filled: false,
            shape_backup: None,
            shape_bounds: None,
            selection: None,
            cursor_px: None,
            current_color: initial_color,
            zoom_percent: 100.,
            scroll: (0., 0.),
            size_input,
            width_input,
            height_input,
            color_picker,
            zoom_slider,
            viewport_size: Size::default(),
            viewport_origin: Rc::new(Cell::new(Point::default())),
            view_image: None,
            view_pixels: (0, 0),
            view_dirty: true,
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![color_sub, size_sub, zoom_sub],
        }
    }

    // zoom factor (1.0 = 100%)
    fn zoom(&self) -> f32 {
        self.zoom_percent / 100.
    }

    fn mark_dirty(&mut self) {
        self.view_dirty = true;
    }

    // move the keyboard focus to the canvas so that the shortcuts work
    fn focus_canvas(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
    }

    // -- view geometry -------------------------------------------------------

    // size of the scrollable content at the given zoom: the canvas plus margins, but never
    // smaller than the viewport (so the canvas can be centered when it is small)
    fn content_size(&self, zoom: f32) -> (f32, f32) {
        let view_w = f32::from(self.viewport_size.width);
        let view_h = f32::from(self.viewport_size.height);
        (
            (self.bitmap.width as f32 * zoom + CANVAS_MARGIN * 2.).max(view_w),
            (self.bitmap.height as f32 * zoom + CANVAS_MARGIN * 2.).max(view_h),
        )
    }

    fn clamp_scroll(&mut self) {
        let (content_w, content_h) = self.content_size(self.zoom());
        let max_x = (content_w - f32::from(self.viewport_size.width)).max(0.);
        let max_y = (content_h - f32::from(self.viewport_size.height)).max(0.);
        let clamped = (self.scroll.0.clamp(0., max_x), self.scroll.1.clamp(0., max_y));
        if clamped != self.scroll {
            self.scroll = clamped;
            self.mark_dirty();
        }
    }

    // top-left corner of the canvas in viewport coordinates (logical pixels);
    // the canvas is centered inside the content
    fn canvas_origin(&self) -> (f32, f32) {
        let zoom = self.zoom();
        let (content_w, content_h) = self.content_size(zoom);
        (
            (content_w - self.bitmap.width as f32 * zoom) / 2. - self.scroll.0,
            (content_h - self.bitmap.height as f32 * zoom) / 2. - self.scroll.1,
        )
    }

    // change the zoom while keeping the canvas point at the viewport center in place
    fn apply_zoom(&mut self, new_percent: f32) {
        let new_percent = new_percent.clamp(MIN_ZOOM_PERCENT, MAX_ZOOM_PERCENT);
        let anchor = (
            f32::from(self.viewport_size.width) / 2.,
            f32::from(self.viewport_size.height) / 2.,
        );

        // the canvas point (in canvas pixels) currently under the anchor
        let old_zoom = self.zoom();
        let (ox, oy) = self.canvas_origin();
        let cx = (anchor.0 - ox) / old_zoom;
        let cy = (anchor.1 - oy) / old_zoom;

        self.zoom_percent = new_percent;
        let new_zoom = self.zoom();

        // choose the scroll so that the same canvas point stays under the anchor
        let (content_w, content_h) = self.content_size(new_zoom);
        self.scroll.0 = (content_w - self.bitmap.width as f32 * new_zoom) / 2. - (anchor.0 - cx * new_zoom);
        self.scroll.1 = (content_h - self.bitmap.height as f32 * new_zoom) / 2. - (anchor.1 - cy * new_zoom);
        self.clamp_scroll();
        self.mark_dirty();
    }

    // zoom so that the whole canvas fits into the viewport
    fn fit_to_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let avail_w = f32::from(self.viewport_size.width) - CANVAS_MARGIN * 2.;
        let avail_h = f32::from(self.viewport_size.height) - CANVAS_MARGIN * 2.;
        let zoom = (avail_w / self.bitmap.width as f32).min(avail_h / self.bitmap.height as f32);
        // round down to a whole percent so that the canvas is never cut off
        let percent = (zoom * 100.).floor().clamp(MIN_ZOOM_PERCENT, MAX_ZOOM_PERCENT);
        self.apply_zoom(percent);
        self.scroll = (0., 0.);
        self.clamp_scroll();
        // keep the slider thumb in sync
        self.zoom_slider
            .update(cx, |state, cx| state.set_value(percent, window, cx));
        cx.notify();
    }

    // convert a window position into canvas pixel coordinates (may be outside the bitmap)
    fn to_canvas_px(&self, pos: Point<Pixels>) -> (i32, i32) {
        let origin = self.viewport_origin.get();
        let (ox, oy) = self.canvas_origin();
        let zoom = self.zoom();
        let x = (f32::from(pos.x - origin.x) - ox) / zoom;
        let y = (f32::from(pos.y - origin.y) - oy) / zoom;
        (x.floor() as i32, y.floor() as i32)
    }

    // -- view rendering ------------------------------------------------------

    // build the visible part of the bitmap, magnified with nearest-neighbor sampling
    fn rebuild_view_image(&mut self, window: &mut Window) {
        let scale = window.scale_factor();
        let w = (f32::from(self.viewport_size.width) * scale).ceil().max(1.) as usize;
        let h = (f32::from(self.viewport_size.height) * scale).ceil().max(1.) as usize;

        // device pixels per canvas pixel, and the canvas origin in device pixels
        let zoom = self.zoom() * scale;
        let (ox, oy) = self.canvas_origin();
        let (ox, oy) = (ox * scale, oy * scale);

        let bw = self.bitmap.width as i32;
        let bh = self.bitmap.height as i32;

        // canvas coordinate of every device column/row (-1 = outside of the canvas)
        let map = |origin: f32, count: usize, limit: i32| -> Vec<i32> {
            (0..count)
                .map(|i| {
                    let c = ((i as f32 + 0.5 - origin) / zoom).floor() as i32;
                    if c < 0 || c >= limit { -1 } else { c }
                })
                .collect()
        };
        let xs = map(ox, w, bw);
        let ys = map(oy, h, bh);

        // lifted selection pixels are drawn on top of the bitmap
        let floating = self
            .selection
            .as_ref()
            .and_then(|s| s.floating.as_ref().map(|f| (s.x, s.y, f)));

        let grid = self.zoom() >= GRID_MIN_ZOOM;
        let mut buf = vec![0u8; w * h * 4];
        for y in 0..h {
            let cy = ys[y];
            // first device row of a canvas pixel row
            let on_hline = grid && cy >= 0 && (y == 0 || ys[y - 1] != cy);
            let row = &mut buf[y * w * 4..(y + 1) * w * 4];
            for x in 0..w {
                let cx = xs[x];
                let [r, g, b] = if cx < 0 || cy < 0 {
                    SURROUND
                } else {
                    let p = match floating {
                        Some((sx, sy, f))
                        if cx >= sx && cx < sx + f.width as i32 && cy >= sy && cy < sy + f.height as i32 =>
                            {
                                f.pixels[(cy - sy) as usize * f.width + (cx - sx) as usize]
                            }
                        _ => self.bitmap.pixels[cy as usize * self.bitmap.width + cx as usize],
                    };
                    let mut c = [p[0], p[1], p[2]];
                    // first device column of a canvas pixel column
                    if grid && (on_hline || x == 0 || xs[x - 1] != cx) {
                        for v in &mut c {
                            *v = ((*v as u16 + 0x80) / 2) as u8;
                        }
                    }
                    c
                };
                // gpui expects BGRA
                row[x * 4] = b;
                row[x * 4 + 1] = g;
                row[x * 4 + 2] = r;
                row[x * 4 + 3] = 0xff;
            }
        }

        // selection outline: a dashed black/white rectangle, one device pixel wide
        if let Some(sel) = &self.selection {
            let x0 = (ox + sel.x as f32 * zoom).round() as i32;
            let y0 = (oy + sel.y as f32 * zoom).round() as i32;
            let x1 = ((ox + (sel.x + sel.w) as f32 * zoom).round() as i32 - 1).max(x0);
            let y1 = ((oy + (sel.y + sel.h) as f32 * zoom).round() as i32 - 1).max(y0);
            let mut put = |x: i32, y: i32, i: i32| {
                if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                    return;
                }
                let v = if (i / 4) % 2 == 0 { 0u8 } else { 255u8 };
                let o = (y as usize * w + x as usize) * 4;
                buf[o] = v;
                buf[o + 1] = v;
                buf[o + 2] = v;
                buf[o + 3] = 0xff;
            };
            for x in x0..=x1 {
                put(x, y0, x - x0);
                put(x, y1, x - x0);
            }
            for y in y0..=y1 {
                put(x0, y, y - y0);
                put(x1, y, y - y0);
            }
        }

        let buffer = image::RgbaImage::from_raw(w as u32, h as u32, buf)
            .expect("buffer size matches the image size");
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
        // release the previous frame's texture, or the atlas would keep growing
        if let Some(old) = self.view_image.replace(image) {
            let _ = window.drop_image(old);
        }
        self.view_pixels = (w, h);
        self.view_dirty = false;
    }

    // -- history -------------------------------------------------------------

    fn push_edit(&mut self, edit: Edit) {
        self.undo_stack.push(edit);
        self.redo_stack.clear();
        if self.undo_stack.len() > MAX_HISTORY {
            self.undo_stack.remove(0);
        }
    }

    // turn the pixels modified since the last entry into one undo step
    fn record_history(&mut self) {
        let Some((x0, y0, x1, y1)) = self.bitmap.dirty.take() else { return };
        let (x, y, w, h) = (x0 as usize, y0 as usize, (x1 - x0) as usize, (y1 - y0) as usize);
        let before = self.committed.read_region(x, y, w, h);
        let after = self.bitmap.read_region(x, y, w, h);
        if before == after {
            return; // nothing actually changed
        }
        self.committed.write_region(x, y, w, h, &after);
        self.push_edit(Edit::Patch { x, y, w, h, before, after });
    }

    fn apply_edit(&mut self, edit: &Edit, undo: bool) {
        match edit {
            Edit::Patch { x, y, w, h, before, after } => {
                let data = if undo { before } else { after };
                self.bitmap.write_region(*x, *y, *w, *h, data);
                self.committed.write_region(*x, *y, *w, *h, data);
            }
            Edit::Replace { before, after } => {
                let target = if undo { before } else { after };
                self.bitmap = target.clone();
                self.committed = target.clone();
            }
        }
        self.bitmap.dirty = None;
    }

    fn after_history_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_size_inputs(window, cx);
        self.clamp_scroll();
        self.mark_dirty();
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // flatten a floating selection first, so that undo cancels the move
        self.commit_selection();
        if let Some(edit) = self.undo_stack.pop() {
            self.apply_edit(&edit, true);
            self.redo_stack.push(edit);
            self.after_history_change(window, cx);
        }
    }

    fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_selection();
        if let Some(edit) = self.redo_stack.pop() {
            self.apply_edit(&edit, false);
            self.undo_stack.push(edit);
            self.after_history_change(window, cx);
        }
    }

    // replace the whole bitmap (resize / open); `record` makes it undoable
    fn replace_bitmap(&mut self, new_bitmap: Bitmap, record: bool) {
        self.selection = None;
        self.drag = Drag::None;
        if record {
            self.record_history();
            let before = std::mem::replace(&mut self.committed, new_bitmap.clone());
            self.push_edit(Edit::Replace { before, after: new_bitmap.clone() });
        } else {
            self.committed = new_bitmap.clone();
            self.undo_stack.clear();
            self.redo_stack.clear();
        }
        self.bitmap = new_bitmap;
    }

    // -- selection -----------------------------------------------------------

    // flatten the selection into the bitmap (a no-op if the pixels were never lifted)
    fn commit_selection(&mut self) {
        let Some(sel) = self.selection.take() else { return };
        if let Some(floating) = sel.floating {
            self.bitmap.paste(&floating, sel.x, sel.y);
            self.record_history();
        }
        self.mark_dirty();
    }

    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = self.selection.take() else { return };
        // lifted pixels already left a hole behind, so only an unlifted selection needs clearing
        if sel.floating.is_none() {
            self.bitmap.fill_rect(sel.x, sel.y, sel.w, sel.h, BACKGROUND);
        }
        self.record_history();
        self.mark_dirty();
        cx.notify();
    }

    fn update_selection(&mut self, start: (i32, i32), end: (i32, i32)) {
        let max_x = self.bitmap.width as i32 - 1;
        let max_y = self.bitmap.height as i32 - 1;
        let clamp = |p: (i32, i32)| (p.0.clamp(0, max_x), p.1.clamp(0, max_y));
        let (s, e) = (clamp(start), clamp(end));
        self.selection = Some(Selection {
            x: s.0.min(e.0),
            y: s.1.min(e.1),
            w: (s.0 - e.0).abs() + 1,
            h: (s.1 - e.1).abs() + 1,
            floating: None,
        });
    }

    fn select_mouse_down(&mut self, p: (i32, i32)) {
        let inside = self.selection.as_ref().is_some_and(|s| s.contains(p));
        if inside {
            let sel = self.selection.as_mut().unwrap();
            // lift the pixels, leaving a background-colored hole
            if sel.floating.is_none() {
                sel.floating = Some(self.bitmap.copy_region(sel.x, sel.y, sel.w, sel.h));
                self.bitmap.fill_rect(sel.x, sel.y, sel.w, sel.h, BACKGROUND);
            }
            self.drag = Drag::SelectMove { grab: (p.0 - sel.x, p.1 - sel.y) };
        } else {
            self.commit_selection();
            self.drag = Drag::SelectNew { start: p };
        }
    }

    // -- drawing -------------------------------------------------------------

    // color used by the pencil-like tools
    fn tool_color(&self) -> [u8; 4] {
        match self.tool {
            Tool::Eraser => BACKGROUND,
            _ => to_rgba8(self.current_color),
        }
    }

    // draw (or redraw) the shape preview between two canvas pixels
    fn draw_shape(&mut self, start: (i32, i32), end: (i32, i32)) {
        let Some(backup) = self.shape_backup.as_ref() else { return };
        // erase the previous preview
        if let Some(bounds) = self.shape_bounds {
            self.bitmap.restore_region(backup, bounds);
        }

        let color = to_rgba8(self.current_color);
        let size = self.brush_size;
        let (x0, x1) = (start.0.min(end.0), start.0.max(end.0));
        let (y0, y1) = (start.1.min(end.1), start.1.max(end.1));
        match self.tool {
            Tool::Line => self.bitmap.line(start, end, size, color),
            Tool::Rect => self.bitmap.rect(x0, y0, x1, y1, size, self.shape_filled, color),
            Tool::Oval => self.bitmap.ellipse(x0, y0, x1, y1, size, self.shape_filled, color),
            _ => {}
        }
        // the preview may extend beyond the corners by the brush size
        self.shape_bounds = Some((x0 - size, y0 - size, x1 + size + 1, y1 + size + 1));
    }

    // -- canvas size ---------------------------------------------------------

    fn sync_size_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let width = self.bitmap.width.to_string();
        let height = self.bitmap.height.to_string();
        self.width_input.update(cx, |state, cx| state.set_value(width, window, cx));
        self.height_input.update(cx, |state, cx| state.set_value(height, window, cx));
    }

    fn apply_resize(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.commit_selection();

        let width = self.width_input.read(cx).value().parse::<usize>().ok();
        let height = self.height_input.read(cx).value().parse::<usize>().ok();
        let (Some(width), Some(height)) = (width, height) else {
            show_error("Enter the width and height as whole numbers.");
            return false;
        };
        if width == 0 || height == 0 || width > MAX_CANVAS_SIZE || height > MAX_CANVAS_SIZE {
            show_error(&format!("The size must be between 1 and {MAX_CANVAS_SIZE}."));
            return false;
        }
        if width == self.bitmap.width && height == self.bitmap.height {
            return false;
        }

        // keep the overlapping part anchored at the top-left, fill the rest with the background
        let mut new_bitmap = Bitmap::new(width, height, BACKGROUND);
        let copy_w = width.min(self.bitmap.width);
        let copy_h = height.min(self.bitmap.height);
        let data = self.bitmap.read_region(0, 0, copy_w, copy_h);
        new_bitmap.write_region(0, 0, copy_w, copy_h, &data);

        self.replace_bitmap(new_bitmap, true);
        self.after_history_change(window, cx);
        true
    }

    fn open_resize_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // show the current size in the inputs
        self.sync_size_inputs(window, cx);

        // the dialog builder may run again, so it only captures cheap handles
        let app = cx.entity();
        let width_input = self.width_input.clone();
        let height_input = self.height_input.clone();
        let initial_focus = Cell::new(true);

        window.open_dialog(cx, move |dialog, window, cx| {
            // focus the width field once, when the dialog opens
            if initial_focus.replace(false) {
                width_input.focus_handle(cx).focus(window, cx);
            }

            let row = |label: &'static str, input: &Entity<InputState>| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().w(px(56.)).text_sm().child(label))
                    .child(div().flex_1().child(NumberInput::new(input)))
            };

            let app = app.clone();
            dialog
                .title("Resize canvas")
                .w(px(320.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(row("Width", &width_input))
                        .child(row("Height", &height_input))
                        .child(div().text_xs().child(format!(
                            "1 to {MAX_CANVAS_SIZE} px. The image stays anchored at the top-left."
                        ))),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("resize-ok")
                                .primary()
                                .label("OK")
                                .on_click(move |_, window, cx| {
                                    // close only when the size was valid
                                    let applied = app.update(cx, |this, cx| this.apply_resize(window, cx));
                                    if applied {
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("resize-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    // -- input handlers ------------------------------------------------------

    fn handle_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_canvas(window, cx);
        let p = self.to_canvas_px(event.position);
        let tool = self.tool;
        // every tool except Select flattens an active selection first
        if tool != Tool::Select {
            self.commit_selection();
        }
        match tool {
            Tool::Pencil | Tool::Eraser => {
                self.drag = Drag::Draw;
                self.last_pos = p;
                let color = self.tool_color();
                self.bitmap.stamp(p.0, p.1, self.brush_size, color);
            }
            Tool::Fill => {
                let color = to_rgba8(self.current_color);
                self.bitmap.flood_fill(p.0, p.1, color);
                self.record_history();
            }
            Tool::Picker => {
                if let Some(c) = self.bitmap.get(p.0, p.1) {
                    let color: Hsla = rgb(u32::from_be_bytes([0, c[0], c[1], c[2]])).into();
                    self.current_color = color;
                    // keep the picker's displayed color in sync (does not emit Change)
                    self.color_picker
                        .update(cx, |state, cx| state.set_value(color, window, cx));
                    // like classic Paint, go back to drawing after picking
                    self.tool = Tool::Pencil;
                }
            }
            Tool::Line | Tool::Rect | Tool::Oval => {
                self.drag = Drag::Shape { start: p };
                self.last_pos = p;
                self.shape_backup = Some(self.bitmap.clone());
                self.shape_bounds = None;
                self.draw_shape(p, p);
            }
            Tool::Select => self.select_mouse_down(p),
        }
        self.mark_dirty();
        cx.notify();
    }

    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.to_canvas_px(event.position);

        // cursor position shown in the footer
        let inside = p.0 >= 0 && p.1 >= 0 && p.0 < self.bitmap.width as i32 && p.1 < self.bitmap.height as i32;
        let cursor = inside.then_some(p);
        let mut changed = false;
        if cursor != self.cursor_px {
            self.cursor_px = cursor;
            changed = true;
        }

        match self.drag {
            Drag::None => {}
            Drag::Draw => {
                if p != self.last_pos {
                    let color = self.tool_color();
                    self.bitmap.line(self.last_pos, p, self.brush_size, color);
                    self.last_pos = p;
                    self.mark_dirty();
                    changed = true;
                }
            }
            Drag::Shape { start } => {
                if p != self.last_pos {
                    self.last_pos = p;
                    self.draw_shape(start, p);
                    self.mark_dirty();
                    changed = true;
                }
            }
            Drag::SelectNew { start } => {
                self.update_selection(start, p);
                self.mark_dirty();
                changed = true;
            }
            Drag::SelectMove { grab } => {
                if let Some(sel) = self.selection.as_mut() {
                    sel.x = p.0 - grab.0;
                    sel.y = p.1 - grab.1;
                    self.mark_dirty();
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn handle_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Draw => self.record_history(),
            Drag::Shape { .. } => {
                self.shape_backup = None;
                self.shape_bounds = None;
                self.record_history();
            }
            Drag::SelectNew { .. } => {
                // a plain click (no drag) just clears the selection
                if self.selection.as_ref().is_some_and(|s| s.w == 1 && s.h == 1) {
                    self.selection = None;
                }
            }
            // the move is recorded as one step when the selection is flattened
            Drag::SelectMove { .. } | Drag::None => {}
        }
        self.mark_dirty();
        cx.notify();
    }

    fn handle_scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(20.));
        self.scroll.0 -= f32::from(delta.x);
        self.scroll.1 -= f32::from(delta.y);
        self.clamp_scroll();
        self.mark_dirty();
        cx.notify();
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // ignore keys typed into the input fields
        if !self.focus_handle.is_focused(window) {
            return;
        }
        let key = &event.keystroke;
        if key.modifiers.control {
            match key.key.as_str() {
                "z" if key.modifiers.shift => self.redo(window, cx),
                "z" => self.undo(window, cx),
                "y" => self.redo(window, cx),
                "s" => self.save_png(cx),
                "o" => self.open_png(window, cx),
                _ => {}
            }
        } else if key.key == "delete" {
            self.delete_selection(cx);
        }
    }

    // -- files ---------------------------------------------------------------

    fn save_png(&mut self, cx: &mut Context<Self>) {
        // make sure a moved selection is part of the saved image
        self.commit_selection();
        self.drag = Drag::None;

        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_file_name("drawing.png")
            .save_file()
        else {
            return; // canceled
        };
        let path = if path.extension().is_none() { path.with_extension("png") } else { path };

        let data: Vec<u8> = self.bitmap.pixels.iter().flatten().copied().collect();
        let Some(img) = image::RgbaImage::from_raw(self.bitmap.width as u32, self.bitmap.height as u32, data) else {
            show_error("Failed to build the image.");
            return;
        };
        if let Err(e) = img.save_with_format(&path, image::ImageFormat::Png) {
            show_error(&format!("Failed to save the image: {e}"));
        }
        self.mark_dirty();
        cx.notify();
    }

    fn open_png(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_selection();
        self.drag = Drag::None;

        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .pick_file()
        else {
            return; // canceled
        };
        let img = match image::open(&path) {
            Ok(img) => img.to_rgba8(),
            Err(e) => {
                show_error(&format!("Failed to open the image: {e}"));
                return;
            }
        };

        let (width, height) = img.dimensions();
        // the canvas is opaque, so flatten transparency onto white
        let pixels = img
            .pixels()
            .map(|p| {
                let a = p[3] as u32;
                let blend = |c: u8| ((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
                [blend(p[0]), blend(p[1]), blend(p[2]), 0xff]
            })
            .collect();

        // opening a file starts a new document: the history is cleared
        let bitmap = Bitmap { width: width as usize, height: height as usize, pixels, dirty: None };
        self.replace_bitmap(bitmap, false);
        self.scroll = (0., 0.);
        self.after_history_change(window, cx);
    }

    // -- UI ------------------------------------------------------------------

    fn tool_button(&self, label: &'static str, icon: impl Into<Icon>, tool: Tool, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Toggle::new(label)
                .tooltip(label)
                .icon(icon)
                .small()
                .checked(self.tool == tool)
                .on_click(cx.listener(move |this, _checked, _, cx| {
                    // leaving the selection tool flattens the selection
                    if tool != Tool::Select {
                        this.commit_selection();
                    }
                    this.tool = tool;
                    this.mark_dirty();
                    cx.notify();
                })),
        )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {

        let open_button = Button::new("btn-open")
            .ghost().xsmall().icon(IconName::Folder).tooltip("Open (Ctrl+O)")
            .on_click(cx.listener(|this, _, window, cx| this.open_png(window, cx)));
        let save_button = Button::new("btn-save")
            .ghost().xsmall().icon(IconName::HardDrive).tooltip("Save (Ctrl+S)")
            .on_click(cx.listener(|this, _, _, cx| this.save_png(cx)));
        let undo_button = Button::new("btn-undo")
            .ghost().xsmall().icon(IconName::Undo2).tooltip("Undo (Ctrl+Z)")
            .on_click(cx.listener(|this, _, window, cx| this.undo(window, cx)));
        let redo_button = Button::new("btn-redo")
            .ghost().xsmall().icon(IconName::Redo2).tooltip("Redo (Ctrl+Y)")
            .on_click(cx.listener(|this, _, window, cx| this.redo(window, cx)));

        let files = div()
            .w_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(2.))
            .child(open_button)
            .child(save_button)
            .child(undo_button)
            .child(redo_button);

        // tool buttons: 2 columns
        let tools = div()
            .w_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(2.))
            .child(self.tool_button("Pencil", IconName::Pencil, Tool::Pencil, cx))
            .child(self.tool_button("Eraser", IconName::Eraser, Tool::Eraser, cx))
            .child(self.tool_button("Fill", IconName::PaintBucket, Tool::Fill, cx))
            .child(self.tool_button("Pick", IconName::Pipette, Tool::Picker, cx))
            .child(self.tool_button("Line", IconName::Slash, Tool::Line, cx))
            .child(self.tool_button("Rect", IconName::Square, Tool::Rect, cx))
            .child(self.tool_button("Oval", IconName::Ellipse, Tool::Oval, cx))
            .child(self.tool_button("Select", IconName::SquareDashedMousePointer, Tool::Select, cx));

        // rectangle / ellipse style
        let filled_button = Toggle::new("filled").label("Filled").small().checked(self.shape_filled)
            .on_click(cx.listener(|this, checked, _, cx| {
                this.shape_filled = *checked; cx.notify();
            }));

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
                            // change the drawing color for subsequent operations
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
            .gap(px(8.))
            .p(px(10.))
            .bg(cx.theme().secondary)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(files)
            .child(tools)
            .child(filled_button)
            .child(brush_size)
            .child(current)
            .child(grid)
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let size_text = format!("{} × {} px", self.bitmap.width, self.bitmap.height);
        let cursor_text = self
            .cursor_px
            .map(|(x, y)| format!("{}, {}", x, y))
            .unwrap_or_default();

        let resize_button = Button::new("btn-resize")
            .xsmall()
            .icon(IconName::ImageUpscale)
            .tooltip("Resize…")
            .on_click(cx.listener(|this, _, window, cx| this.open_resize_dialog(window, cx)));

        let fit_button = Button::new("btn-fit")
            .xsmall()
            .icon(IconName::Maximize)
            .tooltip("Fit to window")
            .on_click(cx.listener(|this, _, window, cx| this.fit_to_window(window, cx)));

        div()
            .w_full()
            .h(px(FOOTER_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .bg(cx.theme().secondary)
            .border_t_1()
            .border_color(cx.theme().sidebar_border)
            // image size and cursor position
            .child(div().w(px(110.)).text_xs().child(size_text))
            .child(div().w(px(80.)).text_xs().child(cursor_text))
            .child(div().flex_1())
            // canvas size
            .child(resize_button)
            // zoom
            .child(div().text_xs().child("Zoom"))
            .child(Slider::new(&self.zoom_slider).w(px(140.)))
            .child(fit_button)
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
            (window_size.width - px(SIDEBAR_WIDTH)).max(px(1.)),
            (window_size.height - px(FOOTER_HEIGHT)).max(px(1.)),
        );
        self.clamp_scroll();

        // rebuild the magnified view only when something changed
        let scale = window.scale_factor();
        let wanted = (
            (f32::from(self.viewport_size.width) * scale).ceil().max(1.) as usize,
            (f32::from(self.viewport_size.height) * scale).ceil().max(1.) as usize,
        );
        if self.view_dirty || self.view_pixels != wanted || self.view_image.is_none() {
            self.rebuild_view_image(window);
        }

        let image = self.view_image.clone();
        let viewport_origin = self.viewport_origin.clone();
        let sidebar = self.render_sidebar(cx);
        let footer = self.render_footer(cx);

        // viewport: shows the pre-rendered image; the handlers live here so a stroke
        // keeps following the mouse even when it leaves the canvas
        let viewport = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .bg(rgb(0xc8c8c8))
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_scroll_wheel(cx.listener(Self::handle_scroll_wheel))
            // the image is generated at device resolution, so it is shown 1:1
            .children(image.map(|image| {
                img(ImageSource::Render(image))
                    .size_full()
                    .object_fit(ObjectFit::Fill)
            }))
            // an invisible overlay that only records the viewport origin
            .child(
                canvas(
                    move |bounds, _, _| viewport_origin.set(bounds.origin),
                    |_, _, _, _| {},
                )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
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

// convert a gpui color into an opaque RGBA pixel
fn to_rgba8(color: Hsla) -> [u8; 4] {
    let c: Rgba = color.into();
    let q = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    [q(c.r), q(c.g), q(c.b), 0xff]
}

fn show_error(message: &str) {
    // the release build has no console, so report errors with a dialog
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Error")
        .set_description(message)
        .show();
}

fn main() {

    application().with_assets(AllAssets).run(move |cx| {

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
    let bounds = Bounds::centered(None, size(px(900.), px(640.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
}
