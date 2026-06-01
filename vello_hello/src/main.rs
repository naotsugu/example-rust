use std::sync::Arc;
use std::error::Error;
use vello::kurbo::{Affine, RoundedRect, Stroke};
use vello::peniko::Color;
use vello::peniko::color::palette;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use vello::wgpu::{self, CurrentSurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::Window;

enum State {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

pub struct App {
    state: State,
    render_context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    scene: Scene,
}

impl ApplicationHandler for App {

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let State::Suspended(cached_window) = &mut self.state else {
            return;
        };

        // get the winit window cached in a previous suspended event or else create a new window
        let window = cached_window
            .take()
            .unwrap_or_else(|| {
                // creates a winit window (wrapped in an Arc for sharing between threads)
                let attr = Window::default_attributes()
                    .with_inner_size(LogicalSize::new(600, 400))
                    .with_resizable(true)
                    .with_title("vello");
                Arc::new(event_loop.create_window(attr).unwrap())
            });

        // create a vello surface
        let size = window.inner_size();
        let surface_future = self.render_context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("error creating surface");

        // create a vello renderer for the surface (using its device id)
        self.renderers
            .resize_with(self.render_context.devices.len(), || None);
        self.renderers[surface.dev_id].get_or_insert_with(|| {
            Renderer::new(
                &self.render_context.devices[surface.dev_id].device,
                RendererOptions::default(),
            ).expect("Couldn't create renderer")
        });

        // save the window and surface to a state variable
        self.state = State::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        };
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // only process events for our window, and only when we have a surface.
        let (surface, valid_surface, window) = match &mut self.state {
            State::Active {
                surface,
                valid_surface,
                window,
            } if window.id() == window_id => (surface, valid_surface, window),
            _ => return,
        };

        match event {

            // exit the event loop when a close is requested
            WindowEvent::CloseRequested => event_loop.exit(),

            // resize the surface when the window is resized
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    self.render_context
                        .resize_surface(surface, size.width, size.height);
                    *valid_surface = true;
                } else {
                    *valid_surface = false;
                }
            }

            // this is where all the rendering happens
            WindowEvent::RedrawRequested => {
                if !*valid_surface {
                    return;
                }

                // empty the scene of objects to draw. You could create a new Scene each time,
                // but in this case the same Scene is reused so that the underlying memory allocation
                // can also be reused.
                self.scene.reset();
                // re-add the objects to draw to the scene.
                add_shapes_to_scene(&mut self.scene);

                let width = surface.config.width;
                let height = surface.config.height;
                let device_handle = &self.render_context.devices[surface.dev_id];
                // render to a texture, which we will later copy into the surface
                self.renderers[surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render_to_texture(
                        &device_handle.device,
                        &device_handle.queue,
                        &self.scene,
                        &surface.target_view,
                        &vello::RenderParams {
                            base_color: palette::css::BLACK,
                            width,
                            height,
                            antialiasing_method: AaConfig::Msaa16,
                        },
                    )
                    .expect("failed to render to surface");

                // get the surface's texture
                let surface_texture = match surface.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
                    CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                        self.render_context.configure_surface(surface);
                        window.request_redraw();
                        return;
                    }
                    CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => {
                        window.request_redraw();
                        return;
                    }
                    CurrentSurfaceTexture::Lost => panic!("Surface was lost"),
                    CurrentSurfaceTexture::Validation => {
                        panic!("Validation error getting surface")
                    }
                };

                // perform the copy
                let mut encoder =
                    device_handle
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Surface Blit"),
                        });
                surface.blitter.copy(
                    &device_handle.device,
                    &mut encoder,
                    &surface.target_view,
                    &surface_texture
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                );
                device_handle.queue.submit([encoder.finish()]);
                // queue the texture to be presented on the surface
                surface_texture.present();

                device_handle.device.poll(wgpu::PollType::Poll).unwrap();
            }
            _ => {}
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let State::Active { window, .. } = &self.state {
            self.state = State::Suspended(Some(window.clone()));
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App {
        render_context: RenderContext::new(),
        renderers: vec![],
        state: State::Suspended(None),
        scene: Scene::new(),
    };

    let event_loop = EventLoop::new()?;
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");

    Ok(())
}

fn add_shapes_to_scene(scene: &mut Scene) {
    // draw an outlined rectangle
    let stroke = Stroke::new(6.0);
    let rect = RoundedRect::new(10.0, 10.0, 240.0, 240.0, 20.0);
    let rect_stroke_color = Color::new([0.9804, 0.702, 0.5294, 1.]);
    scene.stroke(&stroke, Affine::IDENTITY, rect_stroke_color, None, &rect);
}
