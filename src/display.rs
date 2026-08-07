use std::collections::HashMap;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Gdi::*;
use windows::core::PCWSTR;

pub const ORIENTATIONS: [(u32, &str); 4] = [
    (0, "Landscape"),
    (1, "Portrait"),
    (2, "Landscape flipped"),
    (3, "Portrait flipped"),
];

pub const SCALINGS: [(u32, &str); 3] = [(0, "Default"), (1, "Stretched"), (2, "Centered")];

pub const DPI_SCALES: [u32; 12] = [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

const DPI_GET: i32 = -3;
const DPI_SET: i32 = -4;
const DPI_TRIES: u32 = 8;
const DPI_WAIT: Duration = Duration::from_millis(40);

#[repr(C)]
struct DpiRange {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    lowest: i32,
    current: i32,
    highest: i32,
}

#[repr(C)]
struct DpiPick {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    index: i32,
}

#[derive(Clone, Copy)]
pub struct Source {
    adapter: LUID,
    id: u32,
}

struct Info {
    source: Source,
    name: String,
    native: Option<(u32, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Mode {
    pub w: u32,
    pub h: u32,
    pub hz: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub w: u32,
    pub h: u32,
    pub hz: u32,
    pub orientation: u32,
    pub scaling: u32,
    pub dpi: u32,
}

#[derive(Clone)]
pub struct Display {
    pub device: String,
    pub id: String,
    pub name: String,
    pub number: u32,
    pub primary: bool,
    pub synthetic: bool,
    pub current: Mode,
    pub orientation: u32,
    pub scaling: u32,
    pub dpi: u32,
    pub dpi_choices: Vec<u32>,
    pub sizes: Vec<(u32, u32)>,
    pub recommended: Target,
    rates_by_size: Vec<((u32, u32), Vec<u32>)>,
}

impl Display {
    pub fn rates(&self, w: u32, h: u32) -> &[u32] {
        self.rates_by_size
            .iter()
            .find(|(size, _)| *size == (w, h))
            .map(|(_, rates)| rates.as_slice())
            .unwrap_or_default()
    }

    pub fn target(&self) -> Target {
        Target {
            w: self.current.w,
            h: self.current.h,
            hz: self.current.hz,
            orientation: self.orientation,
            scaling: self.scaling,
            dpi: self.dpi,
        }
    }
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn portrait(orientation: u32) -> bool {
    orientation == 1 || orientation == 3
}

fn blank() -> DEVMODEW {
    DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    }
}

fn settings_of(device: PCWSTR, which: ENUM_DISPLAY_SETTINGS_MODE) -> Option<DEVMODEW> {
    let mut dm = blank();
    let ok = unsafe { EnumDisplaySettingsW(device, which, &mut dm).as_bool() };
    ok.then_some(dm)
}

fn settings(device: &str, which: ENUM_DISPLAY_SETTINGS_MODE) -> Option<DEVMODEW> {
    let dev = wide(device);
    settings_of(PCWSTR(dev.as_ptr()), which)
}

fn position_of(dm: &DEVMODEW) -> (i32, i32) {
    let at = unsafe { dm.Anonymous1.Anonymous2.dmPosition };
    (at.x, at.y)
}

fn adapters() -> impl Iterator<Item = DISPLAY_DEVICEW> {
    (0..).map_while(|index| {
        let mut dd = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        unsafe { EnumDisplayDevicesW(PCWSTR::null(), index, &mut dd, 0) }
            .as_bool()
            .then_some(dd)
    })
}

fn header(kind: i32, size: usize, source: Source) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(kind),
        size: size as u32,
        adapterId: source.adapter,
        id: source.id,
    }
}

fn sources() -> HashMap<String, Info> {
    let mut out = HashMap::new();
    let (mut n_paths, mut n_modes) = (0u32, 0u32);

    unsafe {
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_paths, &mut n_modes).is_err() {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); n_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modes as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut n_paths,
            paths.as_mut_ptr(),
            &mut n_modes,
            modes.as_mut_ptr(),
            None,
        )
        .is_err()
        {
            return out;
        }

        for path in &paths[..n_paths as usize] {
            let source = Source {
                adapter: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            };
            let mut gdi = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: header(
                    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME.0,
                    size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(),
                    source,
                ),
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut gdi.header) != 0 {
                continue;
            }
            let mut monitor = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: header(
                    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME.0,
                    size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(),
                    Source {
                        adapter: path.targetInfo.adapterId,
                        id: path.targetInfo.id,
                    },
                ),
                ..Default::default()
            };
            let name = if DisplayConfigGetDeviceInfo(&mut monitor.header) == 0 {
                from_wide(&monitor.monitorFriendlyDeviceName)
            } else {
                String::new()
            };

            let mut preferred = DISPLAYCONFIG_TARGET_PREFERRED_MODE {
                header: header(
                    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_PREFERRED_MODE.0,
                    size_of::<DISPLAYCONFIG_TARGET_PREFERRED_MODE>(),
                    Source {
                        adapter: path.targetInfo.adapterId,
                        id: path.targetInfo.id,
                    },
                ),
                ..Default::default()
            };
            let native = (DisplayConfigGetDeviceInfo(&mut preferred.header) == 0
                && preferred.width > 0)
                .then_some((preferred.width, preferred.height));

            out.insert(
                from_wide(&gdi.viewGdiDeviceName),
                Info {
                    source,
                    name,
                    native,
                },
            );
        }
    }
    out
}

fn monitor_id(device: &str) -> String {
    let dev = wide(device);
    let mut monitor = DISPLAY_DEVICEW {
        cb: size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    if !unsafe { EnumDisplayDevicesW(PCWSTR(dev.as_ptr()), 0, &mut monitor, 1).as_bool() } {
        return String::new();
    }
    let raw = from_wide(&monitor.DeviceID);
    let raw = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let parts: Vec<&str> = raw.split('#').collect();
    if parts.len() < 3 {
        return String::new();
    }
    format!("{}\\{}\\{}", parts[0], parts[1], parts[2]).to_uppercase()
}

fn number_of(device: &str) -> u32 {
    device
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

fn looks_synthetic(adapter: &str, id: &str) -> bool {
    let a = adapter.to_lowercase();
    id.is_empty()
        || ["virtual", "idd", "indirect", "remote", "dummy", "spacedesk"]
            .iter()
            .any(|k| a.contains(k))
}

fn dpi_range(source: Source) -> Option<(i32, i32, i32)> {
    let mut query = DpiRange {
        header: header(DPI_GET, size_of::<DpiRange>(), source),
        lowest: 0,
        current: 0,
        highest: 0,
    };
    let ok = unsafe { DisplayConfigGetDeviceInfo(&mut query.header) } == 0;
    ok.then_some((query.lowest, query.current, query.highest))
}

fn scale_at(index: i32) -> u32 {
    DPI_SCALES[index.clamp(0, DPI_SCALES.len() as i32 - 1) as usize]
}

pub fn enumerate() -> Vec<Display> {
    let sources = sources();
    let mut out = Vec::new();

    for dd in adapters() {
        if !dd.StateFlags.contains(DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) {
            continue;
        }
        let device = from_wide(&dd.DeviceName);
        let adapter = from_wide(&dd.DeviceString);
        let id = monitor_id(&device);

        let encoded = wide(&device);
        let name = PCWSTR(encoded.as_ptr());
        let Some(now) = settings_of(name, ENUM_CURRENT_SETTINGS) else {
            continue;
        };
        let orientation = unsafe { now.Anonymous1.Anonymous2.dmDisplayOrientation.0 };
        let scaling = unsafe { now.Anonymous1.Anonymous2.dmDisplayFixedOutput.0 };
        let flip = portrait(orientation);

        let mut modes: Vec<Mode> = Vec::with_capacity(256);
        for n in 0.. {
            let Some(dm) = settings_of(name, ENUM_DISPLAY_SETTINGS_MODE(n)) else {
                break;
            };
            if dm.dmBitsPerPel < 32 || dm.dmPelsWidth == 0 || dm.dmDisplayFrequency <= 1 {
                continue;
            }
            let (w, h) = if flip {
                (dm.dmPelsHeight, dm.dmPelsWidth)
            } else {
                (dm.dmPelsWidth, dm.dmPelsHeight)
            };
            modes.push(Mode {
                w,
                h,
                hz: dm.dmDisplayFrequency,
            });
        }
        modes.sort_unstable_by(|a, b| b.cmp(a));
        modes.dedup();

        let mut sizes = Vec::new();
        let mut rates_by_size: Vec<((u32, u32), Vec<u32>)> = Vec::new();
        for m in &modes {
            match rates_by_size.last_mut() {
                Some((size, rates)) if *size == (m.w, m.h) => rates.push(m.hz),
                _ => {
                    sizes.push((m.w, m.h));
                    rates_by_size.push(((m.w, m.h), vec![m.hz]));
                }
            }
        }

        let info = sources.get(&device);
        let source = info.map(|i| i.source);
        let range = source.and_then(dpi_range);
        let (dpi, dpi_choices) = match range {
            Some((low, cur, high)) => (
                scale_at(low.abs() + cur),
                DPI_SCALES[..=(low.abs() + high).clamp(0, DPI_SCALES.len() as i32 - 1) as usize]
                    .to_vec(),
            ),
            None => (0, Vec::new()),
        };

        let (w, h) = if flip {
            (now.dmPelsHeight, now.dmPelsWidth)
        } else {
            (now.dmPelsWidth, now.dmPelsHeight)
        };

        let (nw, nh) = info
            .and_then(|i| i.native)
            .filter(|size| sizes.contains(size))
            .or_else(|| sizes.first().copied())
            .unwrap_or((w, h));
        let recommended = Target {
            w: nw,
            h: nh,
            hz: rates_by_size
                .iter()
                .find(|(size, _)| *size == (nw, nh))
                .and_then(|(_, rates)| rates.iter().max().copied())
                .unwrap_or(60),
            orientation: 0,
            scaling: 0,
            dpi: range.map(|(low, _, _)| scale_at(low.abs())).unwrap_or(0),
        };

        out.push(Display {
            name: info
                .map(|i| i.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| {
                    if adapter.is_empty() {
                        device.clone()
                    } else {
                        adapter.clone()
                    }
                }),
            synthetic: looks_synthetic(&adapter, &id),
            number: number_of(&device),
            id: if id.is_empty() { device.clone() } else { id },
            device,
            primary: dd.StateFlags.contains(DISPLAY_DEVICE_PRIMARY_DEVICE),
            current: Mode {
                w,
                h,
                hz: now.dmDisplayFrequency,
            },
            orientation,
            scaling,
            dpi,
            dpi_choices,
            sizes,
            recommended,
            rates_by_size,
        });
    }
    out.sort_by_key(|d| d.number);
    out
}

fn reason(code: DISP_CHANGE) -> String {
    match code {
        DISP_CHANGE_BADMODE => "the display doesn't support that mode".into(),
        DISP_CHANGE_BADPARAM | DISP_CHANGE_BADFLAGS => "Windows rejected the request".into(),
        DISP_CHANGE_NOTUPDATED => "the change couldn't be saved".into(),
        DISP_CHANGE_RESTART => "that mode needs a restart".into(),
        DISP_CHANGE_FAILED => "the display driver refused the change".into(),
        other => format!("error code {}", other.0),
    }
}

fn layout() -> Vec<(String, i32, i32)> {
    adapters()
        .filter(|dd| dd.StateFlags.contains(DISPLAY_DEVICE_ATTACHED_TO_DESKTOP))
        .filter_map(|dd| {
            let device = from_wide(&dd.DeviceName);
            let dm = settings(&device, ENUM_CURRENT_SETTINGS)?;
            let (x, y) = position_of(&dm);
            Some((device, x, y))
        })
        .collect()
}

fn restore(saved: &[(String, i32, i32)]) {
    let mut moved = false;
    for (device, x, y) in saved {
        let Some(mut dm) = settings(device, ENUM_CURRENT_SETTINGS) else {
            continue;
        };
        if position_of(&dm) == (*x, *y) {
            continue;
        }
        dm.Anonymous1.Anonymous2.dmPosition.x = *x;
        dm.Anonymous1.Anonymous2.dmPosition.y = *y;
        dm.dmFields = DM_POSITION;
        let dev = wide(device);
        unsafe {
            ChangeDisplaySettingsExW(
                PCWSTR(dev.as_ptr()),
                Some(&dm),
                None,
                CDS_UPDATEREGISTRY | CDS_NORESET,
                None,
            )
        };
        moved = true;
    }
    if moved {
        unsafe { ChangeDisplaySettingsExW(PCWSTR::null(), None, None, CDS_TYPE(0), None) };
    }
}

fn set_mode(device: &str, t: &Target) -> Result<bool, String> {
    let mut dm = settings(device, ENUM_CURRENT_SETTINGS).ok_or("that display isn't connected")?;
    let (w, h) = if portrait(t.orientation) {
        (t.h, t.w)
    } else {
        (t.w, t.h)
    };

    let unchanged = dm.dmPelsWidth == w
        && dm.dmPelsHeight == h
        && dm.dmDisplayFrequency == t.hz
        && unsafe { dm.Anonymous1.Anonymous2.dmDisplayOrientation.0 } == t.orientation
        && unsafe { dm.Anonymous1.Anonymous2.dmDisplayFixedOutput.0 } == t.scaling;
    if unchanged {
        return Ok(false);
    }

    dm.dmPelsWidth = w;
    dm.dmPelsHeight = h;
    dm.dmDisplayFrequency = t.hz;
    dm.dmBitsPerPel = 32;
    dm.Anonymous1.Anonymous2.dmDisplayOrientation = DEVMODE_DISPLAY_ORIENTATION(t.orientation);
    dm.Anonymous1.Anonymous2.dmDisplayFixedOutput = DEVMODE_DISPLAY_FIXED_OUTPUT(t.scaling);
    dm.dmFields = DM_PELSWIDTH
        | DM_PELSHEIGHT
        | DM_DISPLAYFREQUENCY
        | DM_BITSPERPEL
        | DM_DISPLAYORIENTATION
        | DM_DISPLAYFIXEDOUTPUT;

    let dev = wide(device);
    let dev = PCWSTR(dev.as_ptr());
    let trial = unsafe { ChangeDisplaySettingsExW(dev, Some(&dm), None, CDS_TEST, None) };
    if trial != DISP_CHANGE_SUCCESSFUL {
        return Err(reason(trial));
    }
    let done = unsafe { ChangeDisplaySettingsExW(dev, Some(&dm), None, CDS_UPDATEREGISTRY, None) };
    if done != DISP_CHANGE_SUCCESSFUL && done != DISP_CHANGE_RESTART {
        return Err(reason(done));
    }
    Ok(true)
}

pub fn current_target(device: &str) -> Option<Target> {
    let dm = settings(device, ENUM_CURRENT_SETTINGS)?;
    let orientation = unsafe { dm.Anonymous1.Anonymous2.dmDisplayOrientation.0 };
    let (w, h) = if portrait(orientation) {
        (dm.dmPelsHeight, dm.dmPelsWidth)
    } else {
        (dm.dmPelsWidth, dm.dmPelsHeight)
    };
    let dpi = sources()
        .get(device)
        .map(|info| info.source)
        .and_then(dpi_range)
        .map(|(low, current, _)| scale_at(low.abs() + current))
        .unwrap_or(0);
    Some(Target {
        w,
        h,
        hz: dm.dmDisplayFrequency,
        orientation,
        scaling: unsafe { dm.Anonymous1.Anonymous2.dmDisplayFixedOutput.0 },
        dpi,
    })
}

fn set_dpi(device: &str, percent: u32) {
    let Some(wanted) = DPI_SCALES.iter().position(|p| *p == percent) else {
        return;
    };
    let wanted = wanted as i32;
    let mut primed = false;

    for attempt in 0..DPI_TRIES {
        let Some(source) = sources().get(device).map(|info| info.source) else {
            return;
        };
        let Some((low, cur, high)) = dpi_range(source) else {
            sleep(DPI_WAIT);
            continue;
        };

        let recommended = low.abs();
        let highest = recommended + high;
        let current = recommended + cur;

        if wanted > highest && attempt < DPI_TRIES - 1 {
            if !primed {
                let settle = if current == highest { 0 } else { highest };
                let pick = DpiPick {
                    header: header(DPI_SET, size_of::<DpiPick>(), source),
                    index: settle - recommended,
                };
                unsafe { DisplayConfigSetDeviceInfo(&pick.header) };
                primed = true;
            }
            sleep(DPI_WAIT);
            continue;
        }

        let goal = wanted.min(highest);
        if current == goal {
            return;
        }

        let pick = DpiPick {
            header: header(DPI_SET, size_of::<DpiPick>(), source),
            index: goal - recommended,
        };
        if unsafe { DisplayConfigSetDeviceInfo(&pick.header) } != 0 {
            sleep(DPI_WAIT);
            continue;
        }
        if let Some((low, cur, _)) = dpi_range(source)
            && low.abs() + cur == goal
        {
            return;
        }
        sleep(DPI_WAIT);
    }
}

static IN_FLIGHT: Mutex<()> = Mutex::new(());

pub fn apply(device: &str, t: &Target) -> Result<Option<Target>, String> {
    let Ok(_only_one) = IN_FLIGHT.try_lock() else {
        return Err("another display is already being changed".into());
    };
    let previous = current_target(device);
    let before = layout();
    let mode_changed = set_mode(device, t)?;
    if before.len() > 1 {
        restore(&before);
    }
    let dpi_changed = t.dpi > 0 && previous.map(|p| p.dpi) != Some(t.dpi);
    if t.dpi > 0 {
        set_dpi(device, t.dpi);
    }
    Ok(if mode_changed || dpi_changed {
        previous
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_targets_swap_their_sides() {
        assert!(!portrait(0) && !portrait(2));
        assert!(portrait(1) && portrait(3));
    }

    #[test]
    fn monitor_ids_come_out_stable() {
        let raw = r"\\?\DISPLAY#GSM5B09#5&1a2b&0&UID4353#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        let raw = raw.strip_prefix(r"\\?\").unwrap();
        let parts: Vec<&str> = raw.split('#').collect();
        assert_eq!(
            format!("{}\\{}\\{}", parts[0], parts[1], parts[2]).to_uppercase(),
            r"DISPLAY\GSM5B09\5&1A2B&0&UID4353"
        );
    }

    #[test]
    fn dpi_indices_are_relative_to_the_recommended_scale() {
        let (lowest, current): (i32, i32) = (-2, -2);
        assert_eq!(scale_at(lowest.abs() + current), 100);
        assert_eq!(scale_at(lowest.abs()), 150);
        assert_eq!(scale_at(lowest.abs() + 1), 175);
        assert_eq!(scale_at(99), 500);
    }

    #[test]
    fn only_one_display_change_can_run_at_a_time() {
        let held = IN_FLIGHT.try_lock().expect("free to start with");
        assert!(IN_FLIGHT.try_lock().is_err());
        drop(held);
        assert!(IN_FLIGHT.try_lock().is_ok());
    }

    #[test]
    fn displays_are_numbered_the_way_windows_numbers_them() {
        assert_eq!(number_of(r"\\.\DISPLAY1"), 1);
        assert_eq!(number_of(r"\\.\DISPLAY12"), 12);
        assert_eq!(number_of("nonsense"), 0);
    }

    #[test]
    fn rate_lookup_finds_nothing_for_an_unknown_size() {
        let d = Display {
            device: String::new(),
            id: String::new(),
            name: String::new(),
            number: 1,
            primary: false,
            synthetic: false,
            current: Mode {
                w: 1920,
                h: 1080,
                hz: 60,
            },
            orientation: 0,
            scaling: 0,
            dpi: 100,
            dpi_choices: vec![100],
            sizes: vec![(1920, 1080)],
            recommended: Target {
                w: 1920,
                h: 1080,
                hz: 144,
                orientation: 0,
                scaling: 0,
                dpi: 100,
            },
            rates_by_size: vec![((1920, 1080), vec![144, 60])],
        };
        assert_eq!(d.rates(1920, 1080), &[144, 60]);
        assert!(d.rates(800, 600).is_empty());
    }
}
