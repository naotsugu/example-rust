#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use image::GenericImageView;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::path::Path;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let image_path = Path::new("img.png");
    let rgba = image::open(image_path)?.into_rgba8();
    let (width, height) = rgba.dimensions();
    let mut pixel_buffer =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);
    drop(rgba); // the source image is no longer needed

    ui.set_target_image(Image::from_rgba8(pixel_buffer.clone())); // copy on write

    let ui_weak = ui.as_weak();
    ui.on_image_clicked(move |x, y| {
        let ui = ui_weak.unwrap();
        let (x, y) = (x as u32, y as u32);

        // Release the UI's reference first so the buffer is uniquely owned;
        // otherwise make_mut_slice() would copy the whole buffer (copy-on-write)
        ui.set_target_image(Image::default());

        let pixels = pixel_buffer.make_mut_slice();
        pixels[(y * width + x) as usize] = Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 };

        ui.set_target_image(Image::from_rgba8(pixel_buffer.clone()));

    });


    // let image_path = Path::new("img.png");
    // let img = image::open(image_path)?;
    // let (width, height) = img.dimensions();
    //
    // let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
    //     img.as_rgba8().expect("fault image to buffer"), width, height);
    //
    // ui.set_target_image(Image::from_rgba8(pixel_buffer.clone()));
    //
    // let ui_weak = ui.as_weak();
    //
    // ui.on_image_clicked(move |mouse_x, mouse_y| {
    //     let ui = ui_weak.unwrap();
    //
    //     let target_x = mouse_x as u32;
    //     let target_y = mouse_y as u32;
    //
    //     if target_x < width && target_y < height {
    //         let pixels = pixel_buffer.make_mut_slice();
    //         let index = (target_y * width + target_x) as usize;
    //         pixels[index] = Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 };
    //
    //         ui.set_target_image(Image::from_rgba8(pixel_buffer.clone()));
    //     }
    // });

    ui.run()?;

    Ok(())
}
