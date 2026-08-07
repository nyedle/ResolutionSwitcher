use crate::watch;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};
use windows::core::PCWSTR;

const NAMES: [&str; 2] = [
    r"Global\ResolutionSwitcher-7B4C1E2A-9D3F-4A61-B0C8-5E2F1A7D6C43",
    r"Local\ResolutionSwitcher-7B4C1E2A-9D3F-4A61-B0C8-5E2F1A7D6C43",
];

static HELD: AtomicIsize = AtomicIsize::new(0);

pub fn claim() -> bool {
    for name in NAMES {
        match take(name) {
            Taken::Mine(handle) => {
                HELD.store(handle.0 as isize, Ordering::SeqCst);
                return true;
            }
            Taken::Running => {
                wake();
                return false;
            }
            Taken::Refused => continue,
        }
    }
    true
}

pub fn release() {
    let raw = HELD.swap(0, Ordering::SeqCst);
    if raw != 0 {
        let _ = unsafe { CloseHandle(HANDLE(raw as *mut _)) };
    }
}

enum Taken {
    Mine(HANDLE),
    Running,
    Refused,
}

fn take(name: &str) -> Taken {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        match CreateMutexW(None, true, PCWSTR(wide.as_ptr())) {
            Ok(handle) if GetLastError() == ERROR_ALREADY_EXISTS => {
                let _ = CloseHandle(handle);
                Taken::Running
            }
            Ok(handle) => Taken::Mine(handle),
            Err(_) => Taken::Refused,
        }
    }
}

fn wake() {
    let class: Vec<u16> = watch::CLASS
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        if let Ok(window) = FindWindowW(PCWSTR(class.as_ptr()), None) {
            let _ = PostMessageW(Some(window), watch::show_message(), WPARAM(0), LPARAM(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_second_copy_is_turned_away_and_the_name_frees_up_after() {
        let name = r"Local\ResolutionSwitcher-test-2f1c8a";

        let Taken::Mine(first) = take(name) else {
            panic!("the first copy should get the name")
        };
        assert!(matches!(take(name), Taken::Running));

        let _ = unsafe { CloseHandle(first) };

        let Taken::Mine(again) = take(name) else {
            panic!("the name should be free once the holder lets go")
        };
        let _ = unsafe { CloseHandle(again) };
    }
}
