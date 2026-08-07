use std::sync::OnceLock;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

pub const CLASS: &str = "ResolutionSwitcherWatcher";

pub enum Signal {
    DisplaysChanged,
    Show,
}

pub fn show_message() -> u32 {
    static ID: OnceLock<u32> = OnceLock::new();
    *ID.get_or_init(|| unsafe { RegisterWindowMessageW(w!("ResolutionSwitcherShowWindow")) })
}

unsafe extern "system" fn proc(window: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(window, msg, w, l) }
}

pub fn spawn(on: impl Fn(Signal) + Send + 'static) {
    std::thread::spawn(move || unsafe {
        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        let class: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let name = PCWSTR(class.as_ptr());

        let template = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: name,
            ..Default::default()
        };
        if RegisterClassW(&template) == 0 {
            return;
        }
        if CreateWindowExW(
            WINDOW_EX_STYLE(0),
            name,
            name,
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .is_err()
        {
            return;
        }

        let wanted = show_message();
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_DISPLAYCHANGE {
                on(Signal::DisplaysChanged);
            } else if msg.message == wanted {
                on(Signal::Show);
            }
            let _ = DispatchMessageW(&msg);
        }
    });
}
