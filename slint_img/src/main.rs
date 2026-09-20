#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::path::Path;
use std::cell::Cell;
use std::rc::Rc;

slint::include_modules!();

const BRUSH_COLOR: Rgba8Pixel = Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 };

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let image_path = Path::new("img.png");
    let rgba = image::open(image_path)?.into_rgba8();
    let (width, height) = rgba.dimensions();

    // One-time copy into a Slint-owned, reference-counted buffer
    let mut pixel_buffer =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);
    drop(rgba);

    ui.set_target_image(Image::from_rgba8(pixel_buffer.clone()));

    // Previous point of the current stroke
    let last_point: Rc<Cell<Option<(i32, i32)>>> = Rc::new(Cell::new(None));

    ui.on_stroke_point({
        let ui_weak = ui.as_weak();
        let last_point = last_point.clone();
        move |x, y| {
            let ui = ui_weak.unwrap();

            // The first point of a stroke has no predecessor: draw a single dot
            let (prev_x, prev_y) = last_point.get().unwrap_or((x, y));
            last_point.set(Some((x, y)));

            // Release the UI's reference so make_mut_slice() does not copy the buffer
            ui.set_target_image(Image::default());

            let (w, h) = (width as i32, height as i32);
            let pixels = pixel_buffer.make_mut_slice();
            for_each_line_point(prev_x, prev_y, x, y, |px, py| {
                // Clip: the pointer may be outside the image while dragging
                if (0..w).contains(&px) && (0..h).contains(&py) {
                    pixels[(py * w + px) as usize] = BRUSH_COLOR;
                }
            });
            ui.set_target_image(Image::from_rgba8(pixel_buffer.clone()));
        }
    });

    ui.on_stroke_end(move || last_point.set(None));

    ui.run()?;

    Ok(())
}


/// Bresenham's line algorithm.
/// Calls `plot` for every point on the line, including both endpoints.
fn for_each_line_point(x0: i32, y0: i32, x1: i32, y1: i32, mut plot: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let (mut x, mut y) = (x0, y0);
    let mut err = dx + dy;

    loop {
        plot(x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
