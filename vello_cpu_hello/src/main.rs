use std::num::NonZeroU32;
use std::rc::Rc;
use vello_common::pixmap::Pixmap;
use vello_cpu::{RenderContext, RenderSettings, Resources};
use vello_cpu::kurbo::{Rect, Shape};
use vello_cpu::color::palette::css::{BLUE};
use winit::{
    application::ApplicationHandler,
    event::{Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct App {
    render_state: RenderState,
    renderer: RenderContext,
    resources: Resources,
    pixmap: Pixmap,
}

enum RenderState {
    Active {
        window: Rc<Window>,
        surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    },
    Suspended,
}

impl ApplicationHandler for App {

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.render_state, RenderState::Active { .. }) {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(
                self.pixmap.width() as u32,
                self.pixmap.height() as u32,
            ))
            .with_resizable(true)
            .with_title("vello cpu")
            .with_visible(true)
            .with_active(true);

        let window = Rc::new(event_loop.create_window(window_attrs).unwrap());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        self.render_state = RenderState::Active { window, surface };
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let RenderState::Active { window, surface } = &mut self.render_state else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let width = size.width.max(1);
                let height = size.height.max(1);

                surface.resize(
                        NonZeroU32::new(width).unwrap(),
                        NonZeroU32::new(height).unwrap(),
                    ).unwrap();

                self.pixmap.resize(width as u16, height as u16);

                self.renderer = RenderContext::new_with(
                    width as u16,
                    height as u16,
                    self.renderer.render_settings().clone(),
                );
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.renderer.reset();
                self.renderer.set_paint(BLUE);
                self.renderer.fill_path(&Rect::new(25.0, 25.0, 75.0, 75.0).to_path(0.1));
                self.renderer.flush();
                self.renderer.render_to_pixmap(&mut self.resources, &mut self.pixmap);

                // Copy pixmap to window surface
                let mut buffer = surface.buffer_mut().unwrap();
                let pixmap_data = self.pixmap.data();

                // Convert RGBA to BGRA/XRGB format expected by softbuffer
                for (buffer_pixel, pixel) in buffer.iter_mut().zip(pixmap_data.iter()) {
                    // softbuffer expects 0RGB format (little-endian: B, G, R, 0)
                    // Our pixmap is premultiplied RGBA
                    *buffer_pixel = u32::from_le_bytes([pixel.b, pixel.g, pixel.r, 0]);
                }
                buffer.present().unwrap();
            }
            _ => {}
        }
    }
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.render_state = RenderState::Suspended;
    }
}

fn main() {
    let width = 600;
    let height = 400;
    let mut app = App {
        render_state: RenderState::Suspended,
        renderer: RenderContext::new_with(
            width,
            height,
            RenderSettings {
                num_threads: 0, // 0 means use default (number of CPU cores)
                ..Default::default()
            },
        ),
        resources: Resources::new(),
        pixmap: Pixmap::new(width, height),
    };

    let event_loop = EventLoop::new().unwrap();
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}
