use std::borrow::Cow;
use std::sync::Arc;
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill, FontData, Blob};
use vello::{RenderParams, RendererOptions, Scene, Glyph};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

struct App {
    window: Option<Arc<Window>>,
    vello_context: Option<vello::util::RenderContext>,
    vello_renderer: Option<vello::Renderer>,
    surface: Option<vello::util::RenderSurface<'static>>,
    font_cx: parley::FontContext,
    layout_cx: parley::LayoutContext<[u8; 4]>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window_attrs = Window::default_attributes().with_title("Vello Parley Text Example");
        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        let size = window.inner_size();
        let mut vello_context = vello::util::RenderContext::new();

        // 1. システム標準の最適なフォーマット（Bgra8Unorm 等）で通常通り作成
        let surface = pollster::block_on(vello_context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::Fifo,
        )).unwrap();

        let device_handle = &vello_context.devices[surface.dev_id];

        // 2. 重要: レンダラー側が「サーフェス固有のフォーマット」を出力できるように指定します
        let options = RendererOptions {
            use_cpu: false,
            antialiasing_support: vello::AaSupport::all(),
            num_init_threads: std::num::NonZeroUsize::new(4),
            pipeline_cache: None,
        };

        // レンダラーを作成
        let renderer = vello::Renderer::new(&device_handle.device, options).unwrap();

        self.window = Some(window);
        self.vello_context = Some(vello_context);
        self.vello_renderer = Some(renderer);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(vello_context), Some(surface)) = (&mut self.vello_context, &mut self.surface) {
                    vello_context.resize_surface(surface, size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(surface) = &self.surface else { return };
                let Some(vello_context) = &self.vello_context else { return };
                let Some(renderer) = &mut self.vello_renderer else { return };
                let window = self.window.as_ref().unwrap();
                let width = window.inner_size().width;
                let height = window.inner_size().height;

                if width == 0 || height == 0 { return; }

                // --- 4. Parley によるテキストレイアウト構築 ---
                let text_content = "Hello, Vello & Parley!";
                let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, text_content, 1.0, true);

                builder.push_default(parley::style::StyleProperty::FontSize(48.0));
                builder.push_default(parley::style::StyleProperty::Brush([255, 255, 255, 255])); // 白 (RGBA)
                builder.push_default(parley::style::StyleProperty::FontStack(
                    parley::style::FontStack::Source(Cow::Borrowed("sans-serif"))
                ));

                let mut layout = builder.build(text_content);
                layout.break_all_lines(None);

                // --- 5. Vello Scene への描画登録 ---
                let mut scene = Scene::new();

                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    Color::from_rgb8(20, 24, 30),
                    None,
                    &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
                );

                let text_pos = Affine::translate((50.0, 150.0));

                for line in layout.lines() {
                    for item in line.items() {
                        if let parley::PositionedLayoutItem::GlyphRun(glyph_run) = item {
                            let run = glyph_run.run();
                            let font = run.font();

                            let font_blob = Blob::new(Arc::new(font.data.clone()));
                            let vello_font = FontData::new(font_blob, 0);

                            let vello_glyphs = glyph_run.positioned_glyphs().map(|g| Glyph {
                                id: g.id as u32,
                                x: g.x,
                                y: g.y,
                            });

                            let run_brush = glyph_run.style().brush;
                            let text_color = Color::from_rgba8(
                                run_brush[0],
                                run_brush[1],
                                run_brush[2],
                                run_brush[3],
                            );

                            scene.draw_glyphs(&vello_font)
                                .brush(text_color)
                                .hint(true)
                                .transform(text_pos)
                                .font_size(run.font_size())
                                .draw(Fill::NonZero, vello_glyphs);
                        }
                    }
                }

                // --- 6. 画面へのレンダリング (エラー解決のコア部分) ---
                let device_handle = &vello_context.devices[surface.dev_id];
                let surface_texture = surface.surface.get_current_texture().unwrap();

                // 【超重要】Vello が内部ストレージバインディングで要求する Rgba8Unorm 形式としてビューを作成
                let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("vello_view"),
                    format: Some(wgpu::TextureFormat::Rgba8Unorm),
                    ..Default::default()
                });

                // Vello に Rgba8Unorm 扱いとして描画を命令
                renderer.render_to_texture(
                    &device_handle.device,
                    &device_handle.queue,
                    &scene,
                    &view,
                    &RenderParams {
                        base_color: Color::BLACK,
                        width,
                        height,
                        antialiasing_method: vello::AaConfig::Msaa16,
                    },
                ).unwrap();

                surface_texture.present();
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        window: None,
        vello_context: None,
        vello_renderer: None,
        surface: None,
        font_cx: parley::FontContext::new(),
        layout_cx: parley::LayoutContext::new(),
    };

    event_loop.run_app(&mut app).unwrap();
}
