#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use image::GenericImageView;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::path::Path;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let image_path = Path::new("input.png");
    let img = image::open(image_path)?;
    let (width, height) = img.dimensions();

    let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        img.as_rgba8().expect("fault image to buffer"), width, height);

    let slint_image = Image::from_rgba8(pixel_buffer);

    ui.set_target_image(slint_image);

    ui.run()?;

    Ok(())
}
