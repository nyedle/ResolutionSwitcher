use std::path::Path;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

pub fn url(link: &str) -> bool {
    safe(link) && hand_to_windows(link)
}

pub fn folder(path: &Path) -> bool {
    hand_to_windows(&path.display().to_string())
}

fn safe(link: &str) -> bool {
    link.starts_with("https://") && !link.contains(['"', '\n', '\r', '\0'])
}

fn hand_to_windows(what: &str) -> bool {
    let wide: Vec<u16> = what.encode_utf16().chain(std::iter::once(0)).collect();
    let handed = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    handed.0 as isize > 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_https_links_are_opened() {
        assert!(safe("https://github.com/nyedle/ResolutionSwitcher"));

        assert!(!safe("http://example.com"));
        assert!(!safe("file:///C:/Windows/System32/cmd.exe"));
        assert!(!safe(r"C:\Windows\System32\cmd.exe"));
        assert!(!safe("javascript:alert(1)"));
        assert!(!safe("https://example.com\" && calc"));
        assert!(!safe("https://example.com\nmore"));
    }
}
