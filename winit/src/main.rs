use winit::{
    event::{ Event, WindowEvent },
    event_loop::{ ControlFlow, EventLoop },
    window::WindowBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(500, 300))
        .with_title("winit")
        .build(&event_loop).unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
