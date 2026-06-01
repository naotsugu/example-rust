use masonry::properties::types::{CrossAxisAlignment, MainAxisAlignment};
use xilem::winit::error::EventLoopError;
use xilem::view::{FlexExt as _, flex_row, label};
use xilem::{EventLoop, TextAlign, WidgetView, WindowOptions, Xilem};

struct State {
    text: String,
}

fn app_logic(data: &mut State) -> impl WidgetView<State> + use<> {
    flex_row((
        label(format!("Hello {}", data.text))
            .text_size(32.)
            .text_alignment(TextAlign::Center)
            .flex(5.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .main_axis_alignment(MainAxisAlignment::Center)
}

fn main() -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(
        State { text: "xilem".to_string() },
        app_logic,
        WindowOptions::new("xilem app"));
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
