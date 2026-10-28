//! tempmon: hardware temps + per-app FPS, full window or always-on-top overlay.
//!   Ctrl+O / Overlay button   switch to overlay      (or start with --overlay)
//!   Overlay: drag to move, double-click for full window, right-click for menu
//! FPS comes from the fps-probe GNOME Shell extension (fps-probe/), which counts the
//! frames every window presents to the compositor.
mod conf;
mod cpu;
mod fps;
mod gamelog;
mod overlay;
mod sensors;
mod theme;
mod window;

use conf::Conf;
use cpu::CpuStats;
use fps::FpsProbe;
use gamelog::GameLog;
use gtk::glib;
use gtk::prelude::*;
use sensors::{default_pins, discover, Gpus, Sensor};
use std::cell::{OnceCell, RefCell};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

pub const CAPS: &[&str] = &["1.8", "2.0", "2.2", "2.5", "2.8", "3.0", "3.2", "off"];

pub type Shared = Rc<RefCell<State>>;

pub struct State {
    pub conf: Conf,
    pub sensors: Vec<Sensor>,
    pub cpu: CpuStats,
    pub fps: FpsProbe,
    pub pins: Vec<String>,
    gpus: Gpus,
    log: GameLog,
}

impl State {
    fn new() -> Self {
        let conf = Conf::load();
        let sensors = discover();
        let pins = conf.strings("pins").unwrap_or_else(|| default_pins(&sensors));
        State {
            conf,
            sensors,
            cpu: CpuStats::new(),
            fps: FpsProbe::default(),
            pins,
            gpus: Gpus::discover(),
            log: GameLog::new(),
        }
    }

    fn poll(&mut self) {
        self.cpu.poll();
        self.fps.poll();
        for s in &mut self.sensors {
            s.poll();
        }
        let gpu = self.gpus.load();
        self.log.update(&self.fps, &self.cpu, &self.sensors, gpu);
    }

    pub fn toggle_pin(&mut self, key: &str, on: bool) {
        self.pins.retain(|k| k != key);
        if on {
            self.pins.push(key.to_string());
        }
        self.pins.sort();
        let pins = self.pins.clone();
        self.conf.set("pins", pins);
    }
}

pub fn set_cap(ghz: &str) {
    // -n: sudoers lets cpu-cap run without a password; never block the UI on a prompt
    let _ = Command::new("sudo").args(["-n", "/usr/local/bin/cpu-cap", ghz]).status();
}

struct Ui {
    main: window::MainWin,
    overlay: Rc<overlay::Overlay>,
}

impl Ui {
    fn save_geom(&self, st: &Shared) {
        if self.main.win.is_visible() {
            let ((w, h), (x, y)) = (self.main.win.size(), self.main.win.position());
            st.borrow_mut().conf.set("main_geom", format!("{w}x{h}+{x}+{y}"));
        }
    }

    fn set_overlay(&self, st: &Shared, on: bool) {
        if on {
            self.save_geom(st);
            self.main.win.hide();
            self.overlay.rebuild(&st.borrow());
            self.overlay.win.show_all();
        } else {
            self.overlay.win.hide();
            self.main.update(&st.borrow());
            self.main.win.show_all();
        }
    }
}

fn main() {
    // native Wayland can't keep a window above others or let it place itself; XWayland can
    std::env::set_var("GDK_BACKEND", "x11");
    if let Err(e) = gtk::init() {
        eprintln!("tempmon: {e}");
        std::process::exit(1);
    }
    if let Some(s) = gtk::Settings::default() {
        s.set_gtk_application_prefer_dark_theme(true);
    }
    let css = gtk::CssProvider::new();
    if css.load_from_data(theme::CSS.as_bytes()).is_ok() {
        if let Some(screen) = gtk::gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(&screen, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }

    let st: Shared = Rc::new(RefCell::new(State::new()));
    st.borrow_mut().poll();
    let ui: Rc<OnceCell<Ui>> = Rc::new(OnceCell::new());
    let mode = {
        let (ui, st) = (ui.clone(), st.clone());
        Rc::new(move |on: bool| {
            if let Some(u) = ui.get() {
                u.set_overlay(&st, on)
            }
        })
    };
    let quit: Rc<dyn Fn()> = {
        let (ui, st) = (ui.clone(), st.clone());
        Rc::new(move || {
            if let Some(u) = ui.get() {
                u.save_geom(&st);
            }
            st.borrow_mut().log.end();
            gtk::main_quit();
        })
    };
    let (m1, m2) = (mode.clone(), mode.clone());
    let main = window::MainWin::new(&st, Rc::new(move || m1(true)), quit.clone());
    let overlay = overlay::Overlay::new(&st, Rc::new(move || m2(false)), quit);
    let _ = ui.set(Ui { main, overlay });
    mode(std::env::args().any(|a| a == "--overlay"));

    let (u, s) = (ui.clone(), st.clone());
    glib::timeout_add_local(Duration::from_secs(1), move || {
        s.borrow_mut().poll();
        if let Some(ui) = u.get() {
            let st = s.borrow();
            if ui.overlay.win.is_visible() {
                ui.overlay.update(&st)
            } else {
                ui.main.update(&st)
            }
        }
        glib::ControlFlow::Continue
    });
    gtk::main();
}
