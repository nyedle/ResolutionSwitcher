pub use tray_icon::TrayIcon;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

pub enum Click {
    Show,
    Quit,
}

pub fn install(on: impl Fn(Click) + Clone + Send + Sync + 'static) -> Option<TrayIcon> {
    let show = MenuItem::new("Show", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let quit_id: MenuId = quit.id().clone();
    let menu = Menu::new();
    let _ = menu.append_items(&[&show, &PredefinedMenuItem::separator(), &quit]);

    MenuEvent::set_event_handler(Some({
        let on = on.clone();
        move |event: MenuEvent| {
            on(if event.id == quit_id {
                Click::Quit
            } else {
                Click::Show
            });
        }
    }));

    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            on(Click::Show);
        }
    }));

    TrayIconBuilder::new()
        .with_tooltip("Resolution Switcher")
        .with_menu(Box::new(menu))
        .with_icon(glyph())
        .build()
        .ok()
}

fn glyph() -> tray_icon::Icon {
    let side = crate::glyph::SIDE;
    let mut pixels = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            if crate::glyph::lit(x, y) {
                let at = ((y * side + x) * 4) as usize;
                pixels[at..at + 4].copy_from_slice(&[0xe6, 0xe6, 0xe6, 0xff]);
            }
        }
    }
    tray_icon::Icon::from_rgba(pixels, side, side).expect("square rgba")
}
