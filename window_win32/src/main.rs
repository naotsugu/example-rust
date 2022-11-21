use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::{PAINTSTRUCT, BeginPaint, EndPaint, TextOutA},
    Win32::System::LibraryLoader::GetModuleHandleA,
    Win32::UI::WindowsAndMessaging::*,
};

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleA(None)?;
        let window_class = s!("window");

        let wc = WNDCLASSA {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance,
            lpszClassName: window_class,

            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        };

        let atom = RegisterClassA(&wc);

        CreateWindowExA(
            WINDOW_EX_STYLE::default(),
            window_class,
            s!("hello world window"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            instance,
            None,
        );

        let mut message = MSG::default();

        while GetMessageA(&mut message, HWND(0), 0, 0).into() {
            DispatchMessageA(&message);
        }

        Ok(())
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(window, &mut ps);
                TextOutA(hdc, 10, 10, "Hello World".as_bytes());
                EndPaint(window, &mut ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}
