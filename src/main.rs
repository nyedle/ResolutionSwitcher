#![windows_subsystem = "windows"]

mod app;
mod config;
mod display;
mod glyph;
mod hotkeys;
mod only;
mod pick;
mod programs;
mod shell;
mod theme;
mod tray;
mod ui;
mod update;
mod watch;

fn main() -> eframe::Result<()> {
    leave_a_note_if_it_crashes();

    if !only::claim() {
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 720.0])
            .with_resizable(false)
            .with_title("Resolution Switcher")
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Resolution Switcher",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

fn window_icon() -> egui::IconData {
    let side = glyph::SIDE;
    let mut rgba = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            if glyph::lit(x, y) {
                let at = ((y * side + x) * 4) as usize;
                rgba[at..at + 3].copy_from_slice(&glyph::INK);
                rgba[at + 3] = 0xff;
            }
        }
    }
    egui::IconData {
        rgba,
        width: side,
        height: side,
    }
}

fn leave_a_note_if_it_crashes() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let file = config::path().with_file_name("crash.txt");
        let note = format!(
            "Resolution Switcher {} crashed.\n\n{panic}\n\nPlease report this at {}/issues\n",
            env!("CARGO_PKG_VERSION"),
            ui::REPO
        );
        let _ = std::fs::write(file, note);
        previous(panic);
    }));
}
