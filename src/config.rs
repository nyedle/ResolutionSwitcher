use crate::display::{DPI_SCALES, ORIENTATIONS, SCALINGS, Target};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, w};

pub const SLOTS: usize = 12;
const MAX_TEXT: usize = 64;
const PORTABLE_MARKER: &str = "portable.txt";

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const RUN_NAME: PCWSTR = w!("Resolution Switcher");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Slot {
    pub label: String,
    pub enabled: bool,
    pub monitor_id: String,
    pub device: String,
    pub w: u32,
    pub h: u32,
    pub hz: u32,
    pub orientation: u32,
    pub scaling: u32,
    pub dpi: u32,
    pub hotkey: String,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            label: String::new(),
            enabled: true,
            monitor_id: String::new(),
            device: String::new(),
            w: 1920,
            h: 1080,
            hz: 60,
            orientation: 0,
            scaling: 0,
            dpi: 0,
            hotkey: String::new(),
        }
    }
}

impl Slot {
    pub fn target(&self) -> Target {
        Target {
            w: self.w,
            h: self.h,
            hz: self.hz,
            orientation: self.orientation,
            scaling: self.scaling,
            dpi: self.dpi,
        }
    }

    pub fn set_target(&mut self, t: Target) {
        self.w = t.w;
        self.h = t.h;
        self.hz = t.hz;
        self.orientation = t.orientation;
        self.scaling = t.scaling;
        self.dpi = t.dpi;
    }

    pub fn summary(&self) -> String {
        let mut s = format!("{} × {} @ {} Hz", self.w, self.h, self.hz);
        if let Some((_, name)) = ORIENTATIONS
            .get(self.orientation as usize)
            .filter(|_| self.orientation != 0)
        {
            s += &format!("  ·  {name}");
        }
        if let Some((_, name)) = SCALINGS
            .get(self.scaling as usize)
            .filter(|_| self.scaling != 0)
        {
            s += &format!("  ·  {name}");
        }
        if self.dpi != 0 {
            s += &format!("  ·  {}%", self.dpi);
        }
        s
    }

    fn sane(mut self) -> Self {
        self.label = trim(&self.label);
        self.monitor_id = trim(&self.monitor_id);
        self.device = trim(&self.device);
        self.w = self.w.clamp(1, 32768);
        self.h = self.h.clamp(1, 32768);
        self.hz = self.hz.clamp(1, 1000);
        self.orientation = self.orientation.min(ORIENTATIONS.len() as u32 - 1);
        self.scaling = self.scaling.min(SCALINGS.len() as u32 - 1);
        if self.dpi != 0 && !DPI_SCALES.contains(&self.dpi) {
            self.dpi = 0;
        }
        if !self.hotkey.is_empty() && !crate::hotkeys::parses(&self.hotkey) {
            self.hotkey.clear();
        }
        self
    }
}

fn trim(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect::<String>()
        .trim()
        .to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Rule {
    pub program: String,
    pub enabled: bool,
    pub on_open: Option<usize>,
    pub on_close: Option<usize>,
}

impl Rule {
    pub fn matches(&self, program: &str) -> bool {
        self.enabled && !self.program.is_empty() && wanted(&self.program) == program.to_lowercase()
    }

    fn sane(mut self) -> Self {
        self.program = trim(&self.program);
        self.on_open = self.on_open.filter(|slot| *slot < SLOTS);
        self.on_close = self.on_close.filter(|slot| *slot < SLOTS);
        self
    }
}

pub fn wanted(program: &str) -> String {
    let name = program.trim().trim_matches('"').to_lowercase();
    let name = name.rsplit(['\\', '/']).next().unwrap_or(&name);
    if name.is_empty() || name.ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub dark: bool,
    pub minimize_to_tray: bool,
    pub auto_update: bool,
    pub confirm_changes: bool,
    pub panic_hotkey: String,
    pub slots: Vec<Option<Slot>>,
    pub rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dark: true,
            minimize_to_tray: true,
            auto_update: true,
            confirm_changes: true,
            panic_hotkey: "Control+Alt+Shift+KeyR".into(),
            slots: vec![None; SLOTS],
            rules: Vec::new(),
        }
    }
}

pub fn path() -> &'static PathBuf {
    static FILE: OnceLock<PathBuf> = OnceLock::new();
    FILE.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe()
            && let Some(folder) = exe.parent()
            && folder.join(PORTABLE_MARKER).exists()
        {
            return folder.join("config.json");
        }
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(base)
            .join("ResolutionSwitcher")
            .join("config.json")
    })
}

pub fn is_portable() -> bool {
    path().parent().is_some_and(|folder| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|beside| beside == folder))
            .unwrap_or(false)
    })
}

impl Config {
    pub fn load() -> Self {
        let mut cfg: Config = std::fs::read_to_string(path())
            .ok()
            .and_then(|raw| {
                let text = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
                serde_json::from_str(text).ok()
            })
            .unwrap_or_default();
        cfg.slots.truncate(SLOTS);
        cfg.slots.resize(SLOTS, None);
        cfg.slots = cfg
            .slots
            .into_iter()
            .map(|slot| slot.map(Slot::sane))
            .collect();
        cfg.rules.truncate(SLOTS);
        cfg.rules = cfg.rules.into_iter().map(Rule::sane).collect();
        if !cfg.panic_hotkey.is_empty() && !crate::hotkeys::parses(&cfg.panic_hotkey) {
            cfg.panic_hotkey.clear();
        }
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let file = path();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(file, json).map_err(|e| e.to_string())
    }

    pub fn slots_for(&self, opened: &[String], closed: &[String]) -> Vec<usize> {
        let mut wanted = Vec::new();
        for rule in &self.rules {
            if let Some(slot) = rule.on_open
                && opened.iter().any(|p| rule.matches(p))
            {
                wanted.push(slot);
            }
            if let Some(slot) = rule.on_close
                && closed.iter().any(|p| rule.matches(p))
            {
                wanted.push(slot);
            }
        }
        wanted.dedup();
        wanted
    }

    pub fn slot_using(&self, hotkey: &str, except: usize) -> Option<usize> {
        self.slots.iter().enumerate().position(|(index, slot)| {
            index != except && slot.as_ref().is_some_and(|s| s.hotkey == hotkey)
        })
    }

    pub fn clashes_with_panic(&self, hotkey: &str) -> bool {
        !hotkey.is_empty() && self.panic_hotkey == hotkey
    }
}

pub fn runs_on_startup() -> bool {
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_NAME,
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
        .is_ok()
    }
}

pub fn set_run_on_startup(on: bool) -> Result<(), String> {
    if !on {
        return unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_NAME) }
            .ok()
            .map_err(|e| e.message());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let quoted: Vec<u16> = format!("\"{}\"", exe.display())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let bytes = std::mem::size_of_val(quoted.as_slice()) as u32;

    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_NAME,
            REG_SZ.0,
            Some(quoted.as_ptr().cast()),
            bytes,
        )
    }
    .ok()
    .map_err(|e| e.message())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_and_broken_config_files_still_load() {
        let partial: Config = serde_json::from_str(r#"{"slots":[null]}"#).unwrap();
        assert!(partial.dark && partial.minimize_to_tray && partial.auto_update);
        assert!(partial.confirm_changes);
        assert_eq!(partial.panic_hotkey, "Control+Alt+Shift+KeyR");
        assert!(partial.rules.is_empty());

        let older = r#"{"slots":[{"label":"a","monitor_id":"","device":"","w":800,
                    "h":600,"hz":60,"orientation":0,"scaling":0,"hotkey":""}]}"#;
        let upgraded: Config = serde_json::from_str(older).unwrap();
        let slot = upgraded.slots[0].as_ref().unwrap();
        assert_eq!(slot.dpi, 0);
        assert!(slot.enabled);

        assert!(serde_json::from_str::<Config>("{ not json").is_err());
    }

    #[test]
    fn a_notepad_saved_config_still_loads() {
        let saved = "\u{feff}{\"dark\":false}";
        let text = saved.strip_prefix('\u{feff}').unwrap_or(saved);
        let cfg: Config = serde_json::from_str(text).unwrap();
        assert!(!cfg.dark);
        assert!(serde_json::from_str::<Config>(saved).is_err());
    }

    #[test]
    fn summaries_hide_untouched_settings() {
        let mut slot = Slot {
            w: 1920,
            h: 1080,
            hz: 144,
            ..Slot::default()
        };
        assert_eq!(slot.summary(), "1920 × 1080 @ 144 Hz");
        slot.orientation = 1;
        slot.dpi = 125;
        assert_eq!(slot.summary(), "1920 × 1080 @ 144 Hz  ·  Portrait  ·  125%");
    }

    #[test]
    fn a_hotkey_can_only_belong_to_one_slot() {
        let mut cfg = Config::default();
        cfg.slots[0] = Some(Slot {
            hotkey: "Control+Alt+KeyR".into(),
            ..Slot::default()
        });
        assert_eq!(cfg.slot_using("Control+Alt+KeyR", 3), Some(0));
        assert_eq!(cfg.slot_using("Control+Alt+KeyR", 0), None);
        assert_eq!(cfg.slot_using("Control+Alt+KeyQ", 3), None);
    }

    #[test]
    fn a_rule_fires_on_the_way_in_and_on_the_way_out() {
        let mut cfg = Config::default();
        cfg.rules.push(Rule {
            program: "cs2.exe".into(),
            enabled: true,
            on_open: Some(0),
            on_close: Some(1),
        });
        cfg.rules.push(Rule {
            program: "ignored.exe".into(),
            enabled: false,
            on_open: Some(5),
            on_close: None,
        });

        let opened = vec!["CS2.exe".to_string(), "ignored.exe".to_string()];
        assert_eq!(cfg.slots_for(&opened, &[]), vec![0]);
        assert_eq!(cfg.slots_for(&[], &["cs2.exe".to_string()]), vec![1]);
        assert!(cfg.slots_for(&["other.exe".to_string()], &[]).is_empty());

        cfg.rules[0].on_close = None;
        assert!(cfg.slots_for(&[], &["cs2.exe".to_string()]).is_empty());
    }

    #[test]
    fn the_old_summary_would_have_panicked() {
        let bad = Slot {
            orientation: 9,
            ..Slot::default()
        };
        let boom = std::panic::catch_unwind(|| {
            let _ = ORIENTATIONS[bad.orientation as usize].1;
        });
        assert!(boom.is_err(), "indexing out of range must panic");
        bad.summary();
        assert_eq!(bad.sane().orientation, 3);
    }

    #[test]
    fn a_hand_edited_config_cannot_crash_anything() {
        let nasty = Slot {
            label: "a\u{7}b\0c".to_string() + &"x".repeat(500),
            orientation: 9_999,
            scaling: 9_999,
            dpi: 133,
            w: 0,
            h: 4_000_000,
            hz: 0,
            hotkey: "Ctrl+NotAKey".into(),
            ..Slot::default()
        }
        .sane();

        assert!(nasty.label.len() <= MAX_TEXT);
        assert!(!nasty.label.contains('\u{7}'));
        assert!(nasty.hotkey.is_empty());
        assert_eq!(nasty.dpi, 0);
        assert!(nasty.w >= 1 && nasty.h <= 32768 && nasty.hz >= 1);
        assert!((nasty.orientation as usize) < ORIENTATIONS.len());
        assert!((nasty.scaling as usize) < SCALINGS.len());
        nasty.summary();

        let rule = Rule {
            program: "  C:\\games\\Cs2.EXE  ".into(),
            enabled: true,
            on_open: Some(999),
            on_close: Some(1),
        }
        .sane();
        assert!(rule.on_open.is_none());
        assert_eq!(rule.on_close, Some(1));
        assert!(rule.matches("cs2.exe"));
    }

    #[test]
    fn a_name_without_exe_still_matches_the_real_process() {
        let rule = Rule {
            program: "cs2".into(),
            enabled: true,
            ..Rule::default()
        };
        assert!(rule.matches("cs2.exe"));
        assert!(!rule.matches("cs2"));
        assert_eq!(wanted("Notepad"), "notepad.exe");
        assert_eq!(wanted(r"C:\a\b\GAME.exe"), "game.exe");
        assert_eq!(wanted(""), "");
    }

    #[test]
    fn rules_match_the_program_name_whatever_the_case() {
        let rule = Rule {
            program: " CS2.exe ".into(),
            enabled: true,
            ..Rule::default()
        };
        assert!(rule.matches("cs2.exe"));
        assert!(!rule.matches("cs2"));
        assert!(!Rule::default().matches(""));
    }
}
