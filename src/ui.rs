use crate::app::App;
use crate::config::{self, Rule, SLOTS, Slot};
use crate::display::{self, Display, Target};
use crate::hotkeys::{self, Captured};
use crate::shell;
use crate::theme::{self, Palette};
use crate::update::Step;

pub const REPO: &str = "https://github.com/nyedle/ResolutionSwitcher";
const FIELD: f32 = 250.0;
const GAP: f32 = 15.0;
const TIGHT: f32 = 7.0;

fn card<R>(ui: &mut egui::Ui, colours: Palette, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    tinted(ui, colours.surface, body)
}

fn tinted<R>(ui: &mut egui::Ui, fill: egui::Color32, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
        .inner
}

fn strong(ui: &mut egui::Ui, colours: Palette, text: impl Into<String>) {
    ui.label(
        egui::RichText::new(text.into())
            .strong()
            .color(colours.text),
    );
}

fn heading(ui: &mut egui::Ui, colours: Palette, title: &str) {
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .small()
            .color(colours.muted),
    );
}

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    listen_for_hotkey(app);

    let colours = theme::colors(app.cfg.dark);
    footer(app, ui, colours);

    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let displays = app.displays.clone();
            ui.add_space(12.0);
            live_monitor(app, ui, &displays, colours);

            ui.add_space(GAP);
            slots(app, ui, &displays, colours);
            ui.add_space(GAP);
            rules(app, ui, colours);
            ui.add_space(GAP);
        });
    });

    settings(app, ui.ctx());
    confirm(app, ui.ctx(), colours);
}

fn confirm(app: &mut App, ctx: &egui::Context, colours: Palette) {
    let now = ctx.input(|i| i.time);
    let Some(left) = app.countdown(now) else {
        return;
    };
    ctx.request_repaint_after(std::time::Duration::from_millis(100));

    let what = app
        .pending
        .as_ref()
        .map(|p| p.what.clone())
        .unwrap_or_default();
    egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
        ui.set_width(330.0);
        ui.add_space(4.0);
        strong(ui, colours, "Keep these display settings?");
        ui.add_space(6.0);
        ui.small(what);
        ui.add_space(10.0);
        ui.colored_label(
            colours.accent_hover,
            format!("Putting the old ones back in {:.0}s", left.ceil()),
        );
        ui.add_space(4.0);
        ui.small("If your screen went dark, just wait.");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new("Keep").fill(colours.accent))
                .clicked()
            {
                app.keep();
            }
            if ui.button("Put it back").clicked() {
                app.undo();
            }
        });
        ui.add_space(4.0);
    });
}

fn listen_for_hotkey(app: &mut App) {
    let Some(target) = app.capture.as_ref().map(|c| c.slot) else {
        return;
    };
    let name = match target {
        Some(index) => format!("Slot {}", index + 1),
        None => "The panic key".to_string(),
    };

    match app.ctx.input(hotkeys::read) {
        Captured::Nothing => {}
        Captured::Cancelled => app.end_capture(),
        Captured::Cleared => {
            match target {
                Some(index) => {
                    if let Some(stored) = app.cfg.slots[index].as_mut() {
                        stored.hotkey.clear();
                    }
                }
                None => app.cfg.panic_hotkey.clear(),
            }
            app.save();
            app.end_capture();
            app.tell(format!("{name} has no hotkey now."), false);
        }
        Captured::Bound(combo) => {
            if let Some(clash) = taken_by(app, &combo, target) {
                if let Some(capture) = app.capture.as_mut() {
                    capture.hint = Some(format!("{clash} already uses that. Pick another."));
                }
                return;
            }
            let shown = hotkeys::readable(&combo);
            match target {
                Some(index) => {
                    if let Some(stored) = app.cfg.slots[index].as_mut() {
                        stored.hotkey = combo;
                    }
                }
                None => app.cfg.panic_hotkey = combo,
            }
            app.save();
            app.end_capture();
            app.tell(format!("{name} is {shown}."), false);
        }
        Captured::Rejected(hint) => {
            if let Some(capture) = app.capture.as_mut() {
                capture.hint = Some(hint.to_string());
            }
        }
    }
}

fn taken_by(app: &App, combo: &str, target: Option<usize>) -> Option<String> {
    if target.is_some() && app.cfg.clashes_with_panic(combo) {
        return Some("The panic key".to_string());
    }
    let used = app.cfg.slot_using(combo, target.unwrap_or(usize::MAX))?;
    Some(
        app.cfg.slots[used]
            .as_ref()
            .map(|s| s.label.clone())
            .unwrap_or_default(),
    )
}

fn live_monitor(app: &mut App, ui: &mut egui::Ui, displays: &[Display], colours: Palette) {
    let chosen = displays
        .get(app.selected)
        .map(describe)
        .unwrap_or_else(|| "no displays found".to_string());

    let mut target = displays
        .get(app.selected)
        .map(|d| *app.picked.get(&d.device).unwrap_or(&d.target()));

    egui::Grid::new("live")
        .num_columns(2)
        .spacing([10.0, 7.0])
        .show(ui, |ui| {
            ui.label("Monitor");
            egui::ComboBox::from_id_salt("live monitor")
                .width(FIELD)
                .selected_text(chosen)
                .show_ui(ui, |ui| {
                    for (index, d) in displays.iter().enumerate() {
                        ui.selectable_value(&mut app.selected, index, describe(d));
                    }
                });
            ui.end_row();

            if let (Some(d), Some(target)) = (displays.get(app.selected), target.as_mut()) {
                mode_rows(ui, "live", Some(d), target);
            }
        });

    let Some(d) = displays.get(app.selected) else {
        return;
    };

    let target = target.unwrap_or_else(|| d.target());
    app.picked.insert(d.device.clone(), target);

    let idle = app.can_change();
    let changed = target != d.target();
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        let apply = egui::Button::new("Apply").fill(if changed {
            colours.accent
        } else {
            colours.raised
        });
        if ui
            .add_enabled(idle, apply)
            .on_hover_text(if changed {
                "Switch this monitor now"
            } else {
                "Already using these settings"
            })
            .clicked()
        {
            app.start(d.device.clone(), target, d.name.clone());
        }

        if ui
            .button("Save as slot")
            .on_hover_text("Keep these settings so a hotkey or a program can bring them back")
            .clicked()
        {
            match app.cfg.slots.iter().position(|s| s.is_none()) {
                Some(index) => {
                    app.cfg.slots[index] = Some(Slot {
                        label: d.name.clone(),
                        monitor_id: d.id.clone(),
                        device: d.device.clone(),
                        w: target.w,
                        h: target.h,
                        hz: target.hz,
                        orientation: target.orientation,
                        scaling: target.scaling,
                        dpi: target.dpi,
                        ..Slot::default()
                    });
                    app.save();
                    app.expanded = Some(index);
                    app.tell(format!("Saved as slot {}.", index + 1), false);
                }
                None => app.tell(format!("All {SLOTS} slots are full."), true),
            }
        }

        let stock = d.recommended;
        if ui
            .add_enabled(app.busy == 0, egui::Button::new("Reset"))
            .on_hover_text(format!(
                "Put {} back to what Windows recommends: {} × {} at {} Hz, landscape, {}% scale.
Use this if a game or a bad mode left things wrong. It applies straight away.",
                d.name,
                stock.w,
                stock.h,
                stock.hz,
                stock.dpi.max(100)
            ))
            .clicked()
        {
            app.reset(d.device.clone(), stock, d.name.clone());
        }
    });
}

fn slots(app: &mut App, ui: &mut egui::Ui, displays: &[Display], colours: Palette) {
    let used: Vec<usize> = (0..SLOTS).filter(|i| app.cfg.slots[*i].is_some()).collect();

    ui.horizontal(|ui| {
        heading(ui, colours, "Slots");
        if used.is_empty() {
            return;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if app.arming_clear {
                "Really clear all?"
            } else {
                "Clear all"
            };
            let button = egui::Button::new(label).small().fill(if app.arming_clear {
                colours.bad
            } else {
                colours.raised
            });
            let response = ui.add(button);
            if response.clicked() {
                if app.arming_clear {
                    app.cfg.slots = vec![None; SLOTS];
                    app.cfg.rules.iter_mut().for_each(|r| {
                        r.on_open = None;
                        r.on_close = None;
                    });
                    app.save();
                    app.rebind();
                    app.arming_clear = false;
                    app.tell("Cleared every slot.", false);
                } else {
                    app.arming_clear = true;
                }
            } else if !response.hovered() {
                app.arming_clear = false;
            }
        });
    });
    ui.add_space(TIGHT);

    if used.is_empty() {
        card(ui, colours, |ui| {
            ui.weak("Nothing saved yet. Set a monitor up above and save it.");
        });
        return;
    }

    for index in used {
        let live = app.cfg.slots[index]
            .as_ref()
            .is_some_and(|slot| slot.enabled);
        let fill = if live {
            colours.surface
        } else {
            colours.raised.gamma_multiply(0.35)
        };
        tinted(ui, fill, |ui| {
            if !live {
                ui.multiply_opacity(0.6);
            }
            slot_row(app, ui, index, displays, colours);
        });
        ui.add_space(3.0);
    }
}

fn slot_row(
    app: &mut App,
    ui: &mut egui::Ui,
    index: usize,
    displays: &[Display],
    colours: Palette,
) {
    let Some(slot) = app.cfg.slots[index].clone() else {
        return;
    };
    let capturing = app.capture.as_ref().is_some_and(|c| c.slot == Some(index));
    let connected = displays
        .iter()
        .any(|d| d.id == slot.monitor_id || d.device == slot.device);
    let clash = app.refusal(index);
    let open = app.expanded == Some(index);

    let mut enabled = slot.enabled;
    let mut label = slot.label.clone();

    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut enabled, "")
            .on_hover_text("Turn this slot off without deleting it")
            .changed()
        {
            if let Some(stored) = app.cfg.slots[index].as_mut() {
                stored.enabled = enabled;
            }
            app.save();
            app.rebind();
        }
        if ui
            .add(
                egui::TextEdit::singleline(&mut label)
                    .frame(egui::Frame::NONE)
                    .desired_width(150.0),
            )
            .on_hover_text("Rename this slot")
            .changed()
            && let Some(stored) = app.cfg.slots[index].as_mut()
        {
            stored.label = label;
            app.save();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let caption = if capturing {
                hotkeys::preview(ui.input(|i| i.modifiers))
            } else if slot.hotkey.is_empty() {
                "Set hotkey".to_string()
            } else {
                hotkeys::readable(&slot.hotkey)
            };
            let chip = egui::Button::new(caption).small().fill(if capturing {
                colours.accent_hover
            } else if clash.is_some() {
                colours.bad.gamma_multiply(0.5)
            } else if slot.hotkey.is_empty() {
                colours.raised
            } else {
                colours.accent.gamma_multiply(0.45)
            });
            if ui
                .add(chip)
                .on_hover_text("Click, then hold Ctrl or Alt and press a key. Esc cancels.")
                .clicked()
            {
                if capturing {
                    app.end_capture();
                } else {
                    app.begin_capture(Some(index));
                }
            }
        });
    });

    ui.horizontal(|ui| {
        ui.add_space(22.0);
        match app.capture.as_ref().filter(|c| c.slot == Some(index)) {
            Some(capture) => match &capture.hint {
                Some(hint) => {
                    ui.colored_label(colours.bad, egui::RichText::new(hint).small());
                }
                None => {
                    ui.colored_label(
                        colours.accent_hover,
                        egui::RichText::new("listening, Esc cancels, Backspace removes").small(),
                    );
                }
            },
            None => match clash {
                Some(why) => {
                    ui.colored_label(colours.bad, egui::RichText::new(why).small());
                }
                None => {
                    ui.small(slot.summary());
                    if !connected {
                        ui.colored_label(colours.muted, egui::RichText::new("· unplugged").small());
                    }
                }
            },
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new("Delete").small())
                .on_hover_text("Remove this slot")
                .clicked()
            {
                app.cfg.slots[index] = None;
                for rule in &mut app.cfg.rules {
                    if rule.on_open == Some(index) {
                        rule.on_open = None;
                    }
                    if rule.on_close == Some(index) {
                        rule.on_close = None;
                    }
                }
                app.save();
                app.rebind();
                app.expanded = None;
                app.tell(format!("Slot {} deleted.", index + 1), false);
                return;
            }
            if ui
                .add(egui::Button::new(if open { "Close" } else { "Edit" }).small())
                .clicked()
            {
                app.expanded = if open { None } else { Some(index) };
            }
            if ui
                .add_enabled(
                    app.can_change() && connected && slot.enabled,
                    egui::Button::new("Apply").small(),
                )
                .on_hover_text(if connected {
                    "Switch to this now"
                } else {
                    "That monitor isn't plugged in"
                })
                .clicked()
            {
                app.fire(index);
            }
        });
    });

    if !open {
        return;
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        ui.vertical(|ui| {
            let salt = format!("slot{index}");
            let mut owner = displays.iter().position(|d| d.id == slot.monitor_id);
            let mut target = slot.target();

            egui::Grid::new(&salt)
                .num_columns(2)
                .spacing([10.0, 7.0])
                .show(ui, |ui| {
                    ui.label("Monitor");
                    let text = owner
                        .and_then(|i| displays.get(i))
                        .map(describe)
                        .unwrap_or_else(|| format!("{} (unplugged)", slot.label));
                    egui::ComboBox::from_id_salt(format!("{salt}monitor"))
                        .width(FIELD - 24.0)
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            for (i, d) in displays.iter().enumerate() {
                                ui.selectable_value(&mut owner, Some(i), describe(d));
                            }
                        });
                    ui.end_row();

                    mode_rows(ui, &salt, owner.and_then(|i| displays.get(i)), &mut target);
                });

            if let Some(picked) = owner.and_then(|i| displays.get(i))
                && picked.id != slot.monitor_id
                && let Some(stored) = app.cfg.slots[index].as_mut()
            {
                stored.monitor_id = picked.id.clone();
                stored.device = picked.device.clone();
                app.save();
            }
            if target != slot.target()
                && let Some(stored) = app.cfg.slots[index].as_mut()
            {
                stored.set_target(target);
                app.save();
            }
        });
    });
}

fn mode_rows(ui: &mut egui::Ui, salt: &str, display: Option<&Display>, t: &mut Target) {
    let Some(d) = display else {
        ui.label("Resolution");
        ui.weak(format!("{} × {}", t.w, t.h));
        ui.end_row();
        ui.label("Refresh rate");
        ui.weak(format!("{} Hz", t.hz));
        ui.end_row();
        ui.label("Orientation");
        ui.weak(display::ORIENTATIONS[t.orientation.min(3) as usize].1);
        ui.end_row();
        ui.label("Scaling");
        ui.weak(display::SCALINGS[t.scaling.min(2) as usize].1);
        ui.end_row();
        if t.dpi != 0 {
            ui.label("DPI scale");
            ui.weak(format!("{}%", t.dpi));
            ui.end_row();
        }
        return;
    };

    ui.label("Resolution");
    pick(ui, salt, "size", format!("{} × {}", t.w, t.h), |ui| {
        for (w, h) in &d.sizes {
            let here = (*w, *h) == (d.current.w, d.current.h);
            let text = if here {
                format!("{w} × {h}  (now)")
            } else {
                format!("{w} × {h}")
            };
            if ui.selectable_label(t.w == *w && t.h == *h, text).clicked() {
                (t.w, t.h) = (*w, *h);
            }
        }
    });
    ui.end_row();

    let rates = d.rates(t.w, t.h);
    if !rates.is_empty() && !rates.contains(&t.hz) {
        t.hz = rates[0];
    }
    ui.label("Refresh rate");
    pick(ui, salt, "rate", format!("{} Hz", t.hz), |ui| {
        for hz in rates {
            let here = *hz == d.current.hz && (t.w, t.h) == (d.current.w, d.current.h);
            let text = if here {
                format!("{hz} Hz  (now)")
            } else {
                format!("{hz} Hz")
            };
            if ui.selectable_label(t.hz == *hz, text).clicked() {
                t.hz = *hz;
            }
        }
    });
    ui.end_row();

    ui.label("Orientation");
    pick(
        ui,
        salt,
        "rotation",
        display::ORIENTATIONS[t.orientation.min(3) as usize].1,
        |ui| {
            for (value, name) in display::ORIENTATIONS {
                ui.selectable_value(&mut t.orientation, value, name);
            }
        },
    );
    ui.end_row();

    ui.label("Scaling");
    pick(
        ui,
        salt,
        "scaling",
        display::SCALINGS[t.scaling.min(2) as usize].1,
        |ui| {
            for (value, name) in display::SCALINGS {
                ui.selectable_value(&mut t.scaling, value, name);
            }
        },
    )
    .on_hover_text("What the panel does when the mode is not its native one");
    ui.end_row();

    if !d.dpi_choices.is_empty() {
        ui.label("DPI scale");
        pick(ui, salt, "dpi", format!("{}%", t.dpi.max(100)), |ui| {
            for percent in &d.dpi_choices {
                let text = if *percent == d.dpi {
                    format!("{percent}%  (now)")
                } else {
                    format!("{percent}%")
                };
                if ui.selectable_label(t.dpi == *percent, text).clicked() {
                    t.dpi = *percent;
                }
            }
        })
        .on_hover_text("How big Windows draws text and controls");
        ui.end_row();
    }
}

fn rules(app: &mut App, ui: &mut egui::Ui, colours: Palette) {
    ui.horizontal(|ui| {
        heading(ui, colours, "When a program opens");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("Add").small()).clicked() {
                app.cfg.rules.push(Rule {
                    enabled: true,
                    ..Rule::default()
                });
                app.save();
            }
        });
    });
    ui.add_space(TIGHT);

    if app.cfg.rules.is_empty() {
        card(ui, colours, |ui| {
            ui.weak("Nothing watched. Add one to switch when a game starts.");
        });
        return;
    }

    let choices: Vec<(usize, String)> = (0..SLOTS)
        .filter_map(|i| {
            app.cfg.slots[i]
                .as_ref()
                .map(|s| (i, format!("{}. {}", i + 1, s.label)))
        })
        .collect();

    let mut remove = None;
    for index in 0..app.cfg.rules.len() {
        let mut rule = app.cfg.rules[index].clone();
        let before = rule.clone();

        card(ui, colours, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut rule.enabled, "")
                    .on_hover_text("Turn this rule off without deleting it");
                ui.add(
                    egui::TextEdit::singleline(&mut rule.program)
                        .hint_text("game.exe")
                        .desired_width(140.0),
                )
                .on_hover_text("The program's file name, as it shows in Task Manager");
                if ui
                    .add(egui::Button::new("Browse").small())
                    .on_hover_text("Find the program on your PC")
                    .clicked()
                {
                    app.browse_for_program(index);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new("Delete").small())
                        .on_hover_text("Remove this rule")
                        .clicked()
                    {
                        remove = Some(index);
                    }
                });
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.small("opens");
                slot_choice(ui, &format!("open{index}"), &choices, &mut rule.on_open);
                ui.small("closes");
                slot_choice(ui, &format!("close{index}"), &choices, &mut rule.on_close);
            });

            if rule.on_open.is_none() && rule.on_close.is_none() && !rule.program.trim().is_empty()
            {
                ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    ui.colored_label(
                        colours.muted,
                        egui::RichText::new("pick a slot or this rule does nothing").small(),
                    );
                });
            }
        });

        if rule != before {
            app.cfg.rules[index] = rule;
            app.save();
        }
        ui.add_space(3.0);
    }

    if let Some(index) = remove {
        app.cfg.rules.remove(index);
        app.save();
    }
}

fn slot_choice(
    ui: &mut egui::Ui,
    salt: &str,
    choices: &[(usize, String)],
    value: &mut Option<usize>,
) {
    let text = value
        .and_then(|v| choices.iter().find(|(i, _)| *i == v))
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| "do nothing".to_string());
    egui::ComboBox::from_id_salt(salt)
        .width(130.0)
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "do nothing");
            for (index, name) in choices {
                ui.selectable_value(value, Some(*index), name);
            }
        });
}

fn footer(app: &mut App, ui: &mut egui::Ui, colours: Palette) {
    let now = ui.input(|i| i.time);
    if let Some(left) = app.expire_notice(now) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
    if let Some(left) = app.cooling() {
        ui.ctx().request_repaint_after(left);
    }

    egui::Panel::bottom("footer").show(ui, |ui| {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if app.busy > 0 {
                ui.spinner();
                ui.weak("applying");
            } else if let Some(left) = app.cooling() {
                ui.weak(format!(
                    "settling, ready in {:.0}s",
                    left.as_secs_f32().ceil()
                ));
            } else {
                match &app.notice {
                    Some(notice) => {
                        let colour = if notice.bad {
                            colours.bad
                        } else {
                            colours.good
                        };
                        ui.colored_label(colour, &notice.text);
                    }
                    None => {
                        ui.weak(format!(
                            "{} monitors, {} hotkeys",
                            app.displays.len(),
                            app.binder.count()
                        ));
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Settings").clicked() {
                    app.settings_open = !app.settings_open;
                }
            });
        });
        ui.add_space(5.0);
    });
}

fn settings(app: &mut App, ctx: &egui::Context) {
    let colours = theme::colors(app.cfg.dark);
    let mut open = app.settings_open;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(310.0);
            ui.add_space(4.0);

            let mut dark = app.cfg.dark;
            if ui.checkbox(&mut dark, "Dark theme").changed() {
                app.cfg.dark = dark;
                theme::apply(&app.ctx, dark);
                app.save();
            }

            let mut tray = app.cfg.minimize_to_tray;
            if ui
                .checkbox(&mut tray, "Closing minimizes to the tray")
                .changed()
            {
                app.cfg.minimize_to_tray = tray;
                app.save();
            }

            let mut startup = app.startup;
            if ui.checkbox(&mut startup, "Start with Windows").changed() {
                match config::set_run_on_startup(startup) {
                    Ok(()) => app.startup = startup,
                    Err(e) => app.tell(format!("Couldn't change that: {e}"), true),
                }
            }

            panic_row(app, ui, colours);
            ui.add_space(4.0);

            let mut confirm = app.cfg.confirm_changes;
            if ui
                .checkbox(&mut confirm, "15 second safety countdown")
                .on_hover_text(
                    "Every change asks you to keep it and undoes itself if you do not.
Turn this off and a resolution your monitor cannot show will stay.",
                )
                .changed()
            {
                app.cfg.confirm_changes = confirm;
                app.save();
            }

            let mut auto = app.cfg.auto_update;
            if ui
                .checkbox(&mut auto, "Install updates automatically")
                .on_hover_text("Checks GitHub on startup and puts the new version in place")
                .changed()
            {
                app.cfg.auto_update = auto;
                app.save();
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            updates(app, ui, theme::colors(app.cfg.dark));
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            ui.weak(if config::is_portable() {
                "Portable, so slots and settings live next to the exe"
            } else {
                "Slots and settings are saved in"
            });
            let path = config::path();
            ui.add(
                egui::TextEdit::singleline(&mut path.display().to_string())
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Small),
            );
            ui.add_space(4.0);
            if ui.button("Open that folder").clicked()
                && !shell::folder(path.parent().unwrap_or(path))
            {
                app.tell("Could not open that folder.", true);
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            strong(ui, colours, "Resolution Switcher");
            ui.small(format!(
                "version {}, MIT licensed",
                env!("CARGO_PKG_VERSION")
            ));
            ui.add_space(4.0);
            if ui.link("Source and issues").clicked() && !shell::url(REPO) {
                app.tell("Could not open your browser.", true);
            }
            ui.add_space(4.0);
        });
    app.settings_open = open;
}

fn panic_row(app: &mut App, ui: &mut egui::Ui, colours: Palette) {
    let capturing = app.capture.as_ref().is_some_and(|c| c.slot.is_none());
    ui.horizontal(|ui| {
        ui.label("Panic key");
        let caption = if capturing {
            hotkeys::preview(ui.input(|i| i.modifiers))
        } else if app.cfg.panic_hotkey.is_empty() {
            "Set hotkey".to_string()
        } else {
            hotkeys::readable(&app.cfg.panic_hotkey)
        };
        let chip = egui::Button::new(caption).fill(if capturing {
            colours.accent_hover
        } else if app.cfg.panic_hotkey.is_empty() {
            colours.raised
        } else {
            colours.accent.gamma_multiply(0.45)
        });
        if ui
            .add(chip)
            .on_hover_text("Puts every monitor back to default, wherever you are. Esc cancels, Backspace removes.")
            .clicked()
        {
            if capturing {
                app.end_capture();
            } else {
                app.begin_capture(None);
            }
        }
    });
    match app.capture.as_ref().filter(|c| c.slot.is_none()) {
        Some(capture) => {
            let (colour, text) = match &capture.hint {
                Some(hint) => (colours.bad, hint.clone()),
                None => (
                    colours.accent_hover,
                    "listening, Esc cancels, Backspace removes".to_string(),
                ),
            };
            ui.colored_label(colour, egui::RichText::new(text).small());
        }
        None => {
            ui.small("Fixes every monitor at once when you cannot see the screen");
        }
    }
}

fn updates(app: &mut App, ui: &mut egui::Ui, colours: Palette) {
    ui.horizontal(|ui| {
        strong(
            ui,
            colours,
            format!("Version {}", env!("CARGO_PKG_VERSION")),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let busy = matches!(app.update, Some(Step::Checking));
            if ui
                .add_enabled(!busy, egui::Button::new("Check now").small())
                .clicked()
            {
                app.look_for_updates();
            }
        });
    });

    ui.add_space(4.0);
    match &app.update {
        None => ui.small("Not checked yet."),
        Some(Step::Checking) => ui.small("Checking GitHub..."),
        Some(Step::UpToDate) => ui.small("This is the newest release."),
        Some(Step::Found(version)) => ui.small(format!("Downloading {version}...")),
        Some(Step::Failed(why)) => ui.colored_label(colours.bad, egui::RichText::new(why).small()),
        Some(Step::Ready(version)) => {
            let text = format!("Version {version} is installed.");
            let label = ui.colored_label(colours.good, egui::RichText::new(text).small());
            if ui
                .add(egui::Button::new("Restart now").fill(colours.accent))
                .clicked()
            {
                app.restart();
            }
            label
        }
    };
}

fn describe(d: &Display) -> String {
    let mut tags = Vec::new();
    if d.primary {
        tags.push("primary");
    }
    if d.synthetic {
        tags.push("virtual");
    }
    let mut text = format!("{}. {}", d.number, d.name);
    if !tags.is_empty() {
        text += &format!(" ({})", tags.join(", "));
    }
    text
}

fn pick<R>(
    ui: &mut egui::Ui,
    salt: &str,
    kind: &str,
    text: impl Into<egui::WidgetText>,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    egui::ComboBox::from_id_salt(format!("{salt}{kind}"))
        .width(FIELD)
        .selected_text(text)
        .show_ui(ui, body)
        .response
}
