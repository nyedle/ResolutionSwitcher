use crate::config::Slot;
use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;
use std::collections::HashMap;
use std::str::FromStr;

pub enum Captured {
    Nothing,
    Cancelled,
    Cleared,
    Bound(String),
    Rejected(&'static str),
}

#[derive(Default)]
pub struct Binder {
    manager: Option<GlobalHotKeyManager>,
    bound: Vec<HotKey>,
    owners: HashMap<u32, usize>,
    panic: Option<u32>,
}

impl Binder {
    pub fn new() -> Self {
        Self {
            manager: GlobalHotKeyManager::new().ok(),
            ..Self::default()
        }
    }

    pub fn count(&self) -> usize {
        self.bound.len()
    }

    pub fn owner(&self, id: u32) -> Option<usize> {
        self.owners.get(&id).copied()
    }

    pub fn is_panic(&self, id: u32) -> bool {
        self.panic == Some(id)
    }

    pub fn release(&mut self) {
        if let Some(manager) = &self.manager {
            let _ = manager.unregister_all(&self.bound);
        }
        self.bound.clear();
        self.owners.clear();
        self.panic = None;
    }

    pub fn bind(&mut self, slots: &[Option<Slot>], panic: &str) -> Vec<(usize, &'static str)> {
        self.release();
        let Some(manager) = &self.manager else {
            return Vec::new();
        };

        let mut seen: HashMap<&str, u32> = HashMap::new();
        let mut refused = Vec::new();

        if !panic.is_empty()
            && let Ok(key) = HotKey::from_str(panic)
            && manager.register(key).is_ok()
        {
            seen.insert(panic, key.id());
            self.panic = Some(key.id());
            self.bound.push(key);
        }

        for (index, slot) in slots.iter().enumerate() {
            let Some(slot) = slot else { continue };
            if slot.hotkey.is_empty() || !slot.enabled {
                continue;
            }
            if seen.contains_key(slot.hotkey.as_str()) {
                refused.push((index, "another slot already uses that key"));
                continue;
            }
            match HotKey::from_str(&slot.hotkey) {
                Ok(key) if manager.register(key).is_ok() => {
                    seen.insert(&slot.hotkey, key.id());
                    self.owners.insert(key.id(), index);
                    self.bound.push(key);
                }
                _ => refused.push((index, "another program already owns that key")),
            }
        }
        refused
    }
}

pub fn read(input: &egui::InputState) -> Captured {
    let pressed = input.events.iter().find_map(|event| match event {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => Some((*key, *modifiers)),
        _ => None,
    });
    let Some((key, modifiers)) = pressed else {
        return Captured::Nothing;
    };

    match key {
        egui::Key::Escape => return Captured::Cancelled,
        egui::Key::Backspace | egui::Key::Delete => return Captured::Cleared,
        _ => {}
    }

    let Some(code) = code_for(key) else {
        return Captured::Rejected("That key can't be used.");
    };
    if !is_function_key(&code) && !modifiers.ctrl && !modifiers.alt && !modifiers.command {
        return Captured::Rejected("Hold Ctrl or Alt too, or use an F-key on its own.");
    }

    let combo = format!("{}{code}", prefix(modifiers));
    match HotKey::from_str(&combo) {
        Ok(_) => Captured::Bound(combo),
        Err(_) => Captured::Rejected("That key can't be used."),
    }
}

pub fn parses(hotkey: &str) -> bool {
    HotKey::from_str(hotkey).is_ok()
}

pub fn preview(modifiers: egui::Modifiers) -> String {
    let held = prefix(modifiers);
    if held.is_empty() {
        "press keys".to_string()
    } else {
        readable(&format!("{held}..."))
    }
}

pub fn readable(hotkey: &str) -> String {
    hotkey
        .replace("Control", "Ctrl")
        .replace("+Key", "+")
        .replace("+Digit", "+")
        .replace("Arrow", "")
}

fn prefix(modifiers: egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl || modifiers.command {
        parts.push("Control");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("{}+", parts.join("+"))
}

fn is_function_key(code: &str) -> bool {
    code.len() >= 2 && code.starts_with('F') && code[1..].bytes().all(|b| b.is_ascii_digit())
}

fn code_for(key: egui::Key) -> Option<String> {
    let name = format!("{key:?}");
    if name.len() == 1 && name.as_bytes()[0].is_ascii_alphabetic() {
        return Some(format!("Key{name}"));
    }
    if let Some(digit) = name.strip_prefix("Num") {
        return Some(format!("Digit{digit}"));
    }
    Some(
        match name.as_str() {
            "Equals" | "Plus" => "Equal",
            "Backtick" => "Backquote",
            "OpenBracket" => "BracketLeft",
            "CloseBracket" => "BracketRight",
            other => other,
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: egui::Key, modifiers: egui::Modifiers) -> Captured {
        held(key, modifiers, false)
    }

    fn held(key: egui::Key, modifiers: egui::Modifiers, repeat: bool) -> Captured {
        let mut input = egui::InputState::default();
        input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        });
        read(&input)
    }

    fn mods(ctrl: bool, alt: bool, shift: bool) -> egui::Modifiers {
        egui::Modifiers {
            ctrl,
            alt,
            shift,
            ..Default::default()
        }
    }

    #[test]
    fn every_offered_combination_parses() {
        let ctrl_alt = mods(true, true, false);
        let keys = [
            egui::Key::R,
            egui::Key::Num1,
            egui::Key::Space,
            egui::Key::ArrowUp,
            egui::Key::Insert,
            egui::Key::Home,
            egui::Key::PageDown,
            egui::Key::Minus,
            egui::Key::Equals,
            egui::Key::Backtick,
            egui::Key::OpenBracket,
            egui::Key::Comma,
        ];
        for key in keys {
            match press(key, ctrl_alt) {
                Captured::Bound(combo) => {
                    HotKey::from_str(&combo).unwrap_or_else(|e| panic!("{combo} rejected: {e}"));
                }
                _ => panic!("{key:?} should be bindable with Ctrl+Alt"),
            }
        }
    }

    #[test]
    fn function_keys_need_no_modifier_but_letters_do() {
        assert!(matches!(
            press(egui::Key::F9, mods(false, false, false)),
            Captured::Bound(_)
        ));
        assert!(matches!(
            press(egui::Key::R, mods(false, false, false)),
            Captured::Rejected(_)
        ));
        assert!(matches!(
            press(egui::Key::R, mods(false, false, true)),
            Captured::Rejected(_)
        ));
        assert!(matches!(
            press(egui::Key::R, mods(true, false, true)),
            Captured::Bound(_)
        ));
    }

    #[test]
    fn a_key_already_held_when_capture_starts_still_registers() {
        assert!(matches!(
            held(egui::Key::R, mods(true, true, false), true),
            Captured::Bound(_)
        ));
    }

    #[test]
    fn escape_cancels_and_backspace_clears() {
        assert!(matches!(
            press(egui::Key::Escape, mods(false, false, false)),
            Captured::Cancelled
        ));
        assert!(matches!(
            press(egui::Key::Backspace, mods(false, false, false)),
            Captured::Cleared
        ));
        assert!(matches!(
            read(&egui::InputState::default()),
            Captured::Nothing
        ));
    }

    #[test]
    fn combos_read_back_the_way_people_write_them() {
        assert_eq!(readable("Control+Alt+KeyR"), "Ctrl+Alt+R");
        assert_eq!(readable("Control+Digit1"), "Ctrl+1");
        assert_eq!(readable("Alt+ArrowUp"), "Alt+Up");
        assert_eq!(preview(mods(true, true, false)), "Ctrl+Alt+...");
        assert_eq!(preview(mods(false, false, false)), "press keys");
    }
}
