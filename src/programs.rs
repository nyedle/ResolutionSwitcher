use std::collections::HashSet;
use std::time::Duration;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::*;

const EVERY: Duration = Duration::from_millis(1500);

pub fn watch(on_change: impl Fn(Vec<String>, Vec<String>) + Send + 'static) {
    std::thread::spawn(move || {
        let mut before = running();
        loop {
            std::thread::sleep(EVERY);
            let now = running();
            if now.is_empty() {
                continue;
            }
            let opened: Vec<String> = now.difference(&before).cloned().collect();
            let closed: Vec<String> = before.difference(&now).cloned().collect();
            if !opened.is_empty() || !closed.is_empty() {
                on_change(opened, closed);
            }
            before = now;
        }
    });
}

fn running() -> HashSet<String> {
    let mut names = HashSet::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return names;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if !name.is_empty() {
                    names.insert(name.to_lowercase());
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_snapshot_sees_this_test_runner() {
        let names = running();
        assert!(!names.is_empty());
        assert!(names.iter().all(|n| n == &n.to_lowercase()));
        assert!(names.contains("explorer.exe") || names.len() > 5);
    }
}
