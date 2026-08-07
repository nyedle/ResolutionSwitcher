use crate::config::{self, Config, Slot};
use crate::display::{self, Display, Target};
use crate::hotkeys::Binder;
use crate::update::{self, Step};
use crate::watch::Signal;
use crate::{only, pick, programs, theme, tray, ui, watch};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const NOTICE_SECONDS: f64 = 6.0;
pub const CONFIRM_SECONDS: f64 = 15.0;
const COOLDOWN: Duration = Duration::from_millis(2000);

pub enum Action {
    Refresh,
    Show,
    Quit,
    Hotkey(u32),
    Programs(Vec<String>, Vec<String>),
    Update(Step),
    Watch(usize, String),
    Done(Result<Applied, String>),
}

pub struct Applied {
    pub what: String,
    pub device: String,
    pub previous: Option<Target>,
    pub confirmable: bool,
}

pub struct Pending {
    pub what: String,
    pub device: String,
    pub previous: Target,
    pub until: f64,
}

pub struct Notice {
    pub text: String,
    pub bad: bool,
    pub at: f64,
}

pub struct Capture {
    pub slot: Option<usize>,
    pub hint: Option<String>,
}

pub struct App {
    pub ctx: egui::Context,
    pub cfg: Config,
    pub displays: Rc<Vec<Display>>,
    pub picked: HashMap<String, Target>,
    pub selected: usize,
    pub notice: Option<Notice>,
    pub busy: u32,
    pub capture: Option<Capture>,
    pub arming_clear: bool,
    pub expanded: Option<usize>,
    pub refused: Vec<(usize, &'static str)>,
    pub settings_open: bool,
    pub pending: Option<Pending>,
    pub startup: bool,
    last_change: Option<Instant>,
    pub update: Option<Step>,
    pub binder: Binder,
    quitting: bool,
    queue: Arc<Mutex<Vec<Action>>>,
    _tray: Option<tray::TrayIcon>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let queue: Arc<Mutex<Vec<Action>>> = Arc::default();

        let post = {
            let queue = queue.clone();
            let ctx = ctx.clone();
            move |action| {
                queue.lock().unwrap().push(action);
                ctx.request_repaint();
            }
        };

        watch::spawn({
            let post = post.clone();
            move |signal| {
                post(match signal {
                    Signal::DisplaysChanged => Action::Refresh,
                    Signal::Show => Action::Show,
                })
            }
        });

        programs::watch({
            let post = post.clone();
            move |opened, closed| post(Action::Programs(opened, closed))
        });

        global_hotkey::GlobalHotKeyEvent::set_event_handler(Some({
            let post = post.clone();
            move |event: global_hotkey::GlobalHotKeyEvent| {
                if event.state == global_hotkey::HotKeyState::Pressed {
                    post(Action::Hotkey(event.id));
                }
            }
        }));

        let tray = tray::install({
            let post = post.clone();
            move |click| {
                post(match click {
                    tray::Click::Quit => Action::Quit,
                    tray::Click::Show => Action::Show,
                })
            }
        });

        let cfg = Config::load();
        theme::apply(&ctx, cfg.dark);

        let mut app = Self {
            ctx,
            cfg,
            displays: Rc::new(Vec::new()),
            picked: HashMap::new(),
            selected: 0,
            notice: None,
            busy: 0,
            capture: None,
            arming_clear: false,
            expanded: None,
            refused: Vec::new(),
            settings_open: false,
            pending: None,
            startup: config::runs_on_startup(),
            last_change: None,
            update: None,
            binder: Binder::new(),
            quitting: false,
            queue,
            _tray: tray,
        };
        app.refresh();
        app.rebind();
        update::sweep();
        if app.cfg.auto_update {
            app.look_for_updates();
        }
        app
    }

    pub fn browse_for_program(&self, index: usize) {
        let queue = self.queue.clone();
        let ctx = self.ctx.clone();
        pick::program(move |program| {
            queue.lock().unwrap().push(Action::Watch(index, program));
            ctx.request_repaint();
        });
    }

    pub fn look_for_updates(&mut self) {
        if matches!(self.update, Some(Step::Checking)) {
            return;
        }
        self.update = Some(Step::Checking);
        let queue = self.queue.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            update::look(|step| {
                queue.lock().unwrap().push(Action::Update(step));
                ctx.request_repaint();
            });
        });
    }

    pub fn restart(&mut self) {
        only::release();
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        self.quitting = true;
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub fn refresh(&mut self) {
        self.displays = Rc::new(display::enumerate());
        self.picked
            .retain(|device, _| self.displays.iter().any(|d| &d.device == device));
        for d in self.displays.iter() {
            self.picked.entry(d.device.clone()).or_insert(d.target());
        }
        self.selected = self.selected.min(self.displays.len().saturating_sub(1));
    }

    pub fn rebind(&mut self) {
        self.refused = self.binder.bind(&self.cfg.slots, &self.cfg.panic_hotkey);
    }

    pub fn refusal(&self, index: usize) -> Option<&'static str> {
        self.refused
            .iter()
            .find(|(slot, _)| *slot == index)
            .map(|(_, why)| *why)
    }

    pub fn tell(&mut self, text: impl Into<String>, bad: bool) {
        self.notice = Some(Notice {
            text: text.into(),
            bad,
            at: self.ctx.input(|i| i.time),
        });
    }

    pub fn expire_notice(&mut self, now: f64) -> Option<f64> {
        let notice = self.notice.as_ref()?;
        let left = NOTICE_SECONDS - (now - notice.at);
        if left <= 0.0 {
            self.notice = None;
            return None;
        }
        Some(left)
    }

    pub fn save(&mut self) {
        if let Err(e) = self.cfg.save() {
            self.tell(format!("Couldn't save settings: {e}"), true);
        }
    }

    pub fn device_for(&self, slot: &Slot) -> Option<String> {
        self.displays
            .iter()
            .find(|d| d.id == slot.monitor_id)
            .or_else(|| self.displays.iter().find(|d| d.device == slot.device))
            .map(|d| d.device.clone())
    }

    pub fn cooling(&self) -> Option<Duration> {
        wait_left(self.last_change, Instant::now(), COOLDOWN)
    }

    pub fn can_change(&self) -> bool {
        self.busy == 0 && self.cooling().is_none()
    }

    pub fn start(&mut self, device: String, target: Target, what: String) {
        if self.busy > 0 {
            self.tell("One display at a time. Try again in a moment.", true);
            return;
        }
        if let Some(left) = self.cooling() {
            self.tell(
                format!("Too quick. Wait {:.0}s.", left.as_secs_f32().ceil()),
                true,
            );
            return;
        }
        self.run(device, target, what, true);
    }

    pub fn reset(&mut self, device: String, target: Target, name: String) {
        if self.busy > 0 {
            self.tell("One display at a time. Try again in a moment.", true);
            return;
        }
        self.pending = None;
        self.run(device, target, format!("{name} back to default"), false);
    }

    fn run(&mut self, device: String, target: Target, what: String, confirmable: bool) {
        self.busy += 1;
        let queue = self.queue.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                display::apply(&device, &target)
            }));
            let outcome = match attempt {
                Ok(Ok(previous)) => Ok(Applied {
                    what,
                    device,
                    previous,
                    confirmable,
                }),
                Ok(Err(e)) => Err(format!("{what}: {e}.")),
                Err(_) => Err(format!("{what}: the display driver misbehaved.")),
            };
            queue.lock().unwrap().push(Action::Done(outcome));
            ctx.request_repaint();
        });
    }

    pub fn keep(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.tell(format!("Kept {}.", pending.what), false);
        }
    }

    pub fn undo(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.run(
            pending.device,
            pending.previous,
            "Put back the old settings".into(),
            false,
        );
    }

    pub fn countdown(&mut self, now: f64) -> Option<f64> {
        let left = self.pending.as_ref()?.until - now;
        if left <= 0.0 {
            self.undo();
            return None;
        }
        Some(left)
    }

    pub fn fire(&mut self, index: usize) {
        let Some(slot) = self.cfg.slots.get(index).cloned().flatten() else {
            return;
        };
        if !slot.enabled {
            return;
        }
        let Some(device) = self.device_for(&slot) else {
            self.tell(format!("{} isn't connected.", slot.label), true);
            return;
        };
        let what = format!("{}: {}", slot.label, slot.summary());
        self.start(device, slot.target(), what);
    }

    pub fn begin_capture(&mut self, slot: Option<usize>) {
        self.binder.release();
        self.capture = Some(Capture { slot, hint: None });
    }

    pub fn reset_everything(&mut self) {
        if self.busy > 0 {
            return;
        }
        let jobs: Vec<(String, Target)> = self
            .displays
            .iter()
            .map(|d| (d.device.clone(), d.recommended))
            .collect();
        if jobs.is_empty() {
            return;
        }

        self.pending = None;
        self.busy += 1;
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        let queue = self.queue.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let mut fixed = 0;
            for (device, target) in &jobs {
                let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    display::apply(device, target)
                }));
                if matches!(done, Ok(Ok(_))) {
                    fixed += 1;
                }
            }
            let what = if fixed == 1 {
                "Put 1 monitor back to default".to_string()
            } else {
                format!("Put {fixed} monitors back to default")
            };
            queue.lock().unwrap().push(Action::Done(Ok(Applied {
                what,
                device: String::new(),
                previous: None,
                confirmable: false,
            })));
            ctx.request_repaint();
        });
    }

    pub fn end_capture(&mut self) {
        self.capture = None;
        self.rebind();
    }

    fn follow_rules(&mut self, opened: &[String], closed: &[String]) {
        for slot in self.cfg.slots_for(opened, closed) {
            self.fire(slot);
        }
    }

    fn drain(&mut self) {
        let pending = std::mem::take(&mut *self.queue.lock().unwrap());
        for action in pending {
            match action {
                Action::Refresh => self.refresh(),
                Action::Show => {
                    self.ctx
                        .send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Action::Quit => {
                    self.quitting = true;
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Action::Hotkey(id) if self.binder.is_panic(id) => self.reset_everything(),
                Action::Hotkey(id) => {
                    if let Some(index) = self.binder.owner(id) {
                        self.fire(index);
                    }
                }
                Action::Programs(opened, closed) => self.follow_rules(&opened, &closed),
                Action::Watch(index, program) => {
                    if let Some(rule) = self.cfg.rules.get_mut(index) {
                        rule.program = program;
                        self.save();
                    }
                }
                Action::Update(step) => {
                    if let Step::Ready(version) = &step {
                        self.tell(
                            format!("Version {version} installed, restart to use it."),
                            false,
                        );
                    }
                    self.update = Some(step);
                }
                Action::Done(outcome) => {
                    self.busy = self.busy.saturating_sub(1);
                    self.last_change = Some(Instant::now());
                    match outcome {
                        Ok(applied) => {
                            self.tell(&applied.what, false);
                            match applied.previous {
                                Some(previous)
                                    if applied.confirmable && self.cfg.confirm_changes =>
                                {
                                    self.pending = Some(Pending {
                                        what: applied.what,
                                        device: applied.device,
                                        previous,
                                        until: self.ctx.input(|i| i.time) + CONFIRM_SECONDS,
                                    });
                                    self.ctx
                                        .send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                                }
                                _ => {}
                            }
                        }
                        Err(text) => self.tell(text, true),
                    }
                    self.refresh();
                }
            }
        }
    }
}

fn wait_left(last: Option<Instant>, now: Instant, cooldown: Duration) -> Option<Duration> {
    let since = now.checked_duration_since(last?)?;
    cooldown.checked_sub(since).filter(|left| !left.is_zero())
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();

        if !self.quitting
            && self.cfg.minimize_to_tray
            && ctx.input(|i| i.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::draw(self, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_are_spaced_out_but_do_not_block_forever() {
        let cooldown = Duration::from_secs(2);
        let now = Instant::now();

        assert!(wait_left(None, now, cooldown).is_none());

        let just_now = now - Duration::from_millis(500);
        let left = wait_left(Some(just_now), now, cooldown).expect("still cooling");
        assert!(left <= Duration::from_millis(1500) && left > Duration::from_millis(1400));

        let ages_ago = now - Duration::from_secs(30);
        assert!(wait_left(Some(ages_ago), now, cooldown).is_none());

        let exactly = now - cooldown;
        assert!(wait_left(Some(exactly), now, cooldown).is_none());
    }
}
