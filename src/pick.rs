use windows::Win32::UI::Controls::Dialogs::*;
use windows::core::{PCWSTR, PWSTR};

pub fn program(chosen: impl FnOnce(String) + Send + 'static) {
    std::thread::spawn(move || {
        if let Some(name) = ask() {
            chosen(name);
        }
    });
}

fn ask() -> Option<String> {
    let filter: Vec<u16> = "Programs\0*.exe\0\0".encode_utf16().collect();
    let default: Vec<u16> = "exe\0".encode_utf16().collect();
    let title: Vec<u16> = "Pick the program to watch for\0".encode_utf16().collect();
    let mut buffer = [0u16; 520];

    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrDefExt: PCWSTR(default.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER,
        ..Default::default()
    };

    if !unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        return None;
    }

    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    program_name(&String::from_utf16_lossy(&buffer[..end]))
}

fn program_name(path: &str) -> Option<String> {
    let name = path.rsplit(['\\', '/']).next()?.trim();
    let looks_right = name.len() > 4
        && name.to_lowercase().ends_with(".exe")
        && !name.contains(['<', '>', '"', '|', '\0']);
    looks_right.then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_program_name_gets_through() {
        assert_eq!(
            program_name(r"C:\Program Files\Steam\steamapps\common\cs2\game\bin\cs2.exe")
                .as_deref(),
            Some("cs2.exe")
        );
        assert_eq!(program_name(r"D:/Games/RL.EXE").as_deref(), Some("RL.EXE"));

        assert!(program_name(r"C:\stuff\geeg.bin").is_none());
        assert!(program_name(r"C:\stuff\notes.txt").is_none());
        assert!(program_name(".exe").is_none());
        assert!(program_name("").is_none());
        assert!(program_name(r"C:\folder\").is_none());
    }
}
