use crate::conf::parse_pos;
use crate::gamelog::{log_path, read_cap};
use crate::theme::{markup, rgb, BG, DIM, FG};
use crate::{set_cap, Shared, State, CAPS};
use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::process::Command;
use std::rc::Rc;

pub struct Overlay {
    pub win: gtk::Window,
    grid: gtk::Grid,
    cpu: gtk::Label,
    fps: (gtk::Label, gtk::Label),
    rows: RefCell<Vec<(usize, gtk::Label)>>,
    bg_alpha: Rc<Cell<f64>>,
    drag: Cell<(f64, f64)>,
}

fn label(xalign: f32) -> gtk::Label {
    let l = gtk::Label::new(None);
    l.set_xalign(xalign);
    l
}

fn radio_menu(title: &str, options: &[(String, String)], active: &str, on: impl Fn(&str) + 'static) -> gtk::MenuItem {
    let (sub, on) = (gtk::Menu::new(), Rc::new(on));
    let mut first: Option<gtk::RadioMenuItem> = None;
    for (id, text) in options {
        let item = gtk::RadioMenuItem::with_label(text);
        match &first {
            Some(f) => item.join_group(Some(f)),
            None => first = Some(item.clone()),
        }
        item.set_active(id == active);
        let (on, id) = (on.clone(), id.clone());
        item.connect_toggled(move |i| {
            if i.is_active() {
                on(&id)
            }
        });
        sub.append(&item);
    }
    let top = gtk::MenuItem::with_label(title);
    top.set_submenu(Some(&sub));
    top
}

fn draw_bg(alpha: f64, w: &gtk::Window, cr: &cairo::Context) {
    cr.set_operator(cairo::Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    let _ = cr.paint();
    if alpha > 0.0 {
        let (r, g, b) = rgb(BG);
        let (wd, ht, rad) = (w.allocated_width() as f64, w.allocated_height() as f64, 6.0);
        cr.set_source_rgba(r, g, b, alpha);
        cr.new_sub_path();
        cr.arc(wd - rad, rad, rad, -PI / 2.0, 0.0);
        cr.arc(wd - rad, ht - rad, rad, 0.0, PI / 2.0);
        cr.arc(rad, ht - rad, rad, PI / 2.0, PI);
        cr.arc(rad, rad, rad, PI, 1.5 * PI);
        cr.close_path();
        let _ = cr.fill();
    }
    cr.set_operator(cairo::Operator::Over);
}

impl Overlay {
    pub fn new(st: &Shared, on_full: Rc<dyn Fn()>, on_quit: Rc<dyn Fn()>) -> Rc<Self> {
        // override-redirect: the WM can't bury it, so it's on every workspace and above fullscreen apps
        let win = gtk::Window::new(gtk::WindowType::Popup);
        win.set_title("tempmon overlay");
        if let Some(screen) = WidgetExt::screen(&win) {
            win.set_visual(screen.rgba_visual().as_ref());
        }
        win.set_app_paintable(true);
        win.style_context().add_class("tm-overlay");
        win.add_events(
            gdk::EventMask::BUTTON_PRESS_MASK | gdk::EventMask::BUTTON_RELEASE_MASK | gdk::EventMask::BUTTON1_MOTION_MASK,
        );
        let grid = gtk::Grid::new();
        grid.set_column_spacing(12);
        grid.set_margin_start(10);
        grid.set_margin_end(10);
        grid.set_margin_top(6);
        grid.set_margin_bottom(6);
        win.add(&grid);
        let fps = (label(0.0), label(1.0));
        fps.0.set_no_show_all(true);
        fps.1.set_no_show_all(true);

        let conf_alpha = st.borrow().conf.f64("bg_alpha").unwrap_or(0.0);
        let ov = Rc::new(Overlay {
            win,
            grid,
            cpu: label(0.0),
            fps,
            rows: RefCell::new(Vec::new()),
            bg_alpha: Rc::new(Cell::new(conf_alpha)),
            drag: Cell::new((0.0, 0.0)),
        });
        let (x, y) = st.borrow().conf.str("overlay_pos").and_then(parse_pos).unwrap_or((40, 40));
        ov.win.move_(x, y);

        let alpha = ov.bg_alpha.clone();
        ov.win.connect_draw(move |w, cr| {
            draw_bg(alpha.get(), w, cr);
            glib::Propagation::Proceed
        });

        let menu = ov.build_menu(st, on_full.clone(), on_quit);
        let o = ov.clone();
        ov.win.connect_button_press_event(move |w, e| {
            match (e.button(), e.event_type()) {
                (3, _) => menu.popup_at_pointer(Some(&**e)),
                (1, gdk::EventType::DoubleButtonPress) => on_full(),
                (1, _) => {
                    let ((x, y), (rx, ry)) = (w.position(), e.root());
                    o.drag.set((rx - x as f64, ry - y as f64));
                }
                _ => {}
            }
            glib::Propagation::Stop
        });
        let o = ov.clone();
        ov.win.connect_motion_notify_event(move |w, e| {
            let ((dx, dy), (rx, ry)) = (o.drag.get(), e.root());
            w.move_((rx - dx) as i32, (ry - dy) as i32);
            glib::Propagation::Stop
        });
        let s = st.clone();
        ov.win.connect_button_release_event(move |w, _| {
            let (x, y) = w.position();
            s.borrow_mut().conf.set("overlay_pos", format!("+{x}+{y}"));
            glib::Propagation::Stop
        });
        ov.rebuild(&st.borrow());
        ov
    }

    fn build_menu(self: &Rc<Self>, st: &Shared, on_full: Rc<dyn Fn()>, on_quit: Rc<dyn Fn()>) -> gtk::Menu {
        let menu = gtk::Menu::new();
        let add = |text: &str, f: Rc<dyn Fn()>| {
            let item = gtk::MenuItem::with_label(text);
            item.connect_activate(move |_| f());
            menu.append(&item);
        };
        add("Full window", on_full);

        let bg: Vec<(String, String)> =
            [0.0, 0.25, 0.5, 0.85, 1.0].iter().map(|a| (a.to_string(), format!("{}%", (a * 100.0) as i32))).collect();
        let (o, s) = (self.clone(), st.clone());
        menu.append(&radio_menu("Background", &bg, &self.bg_alpha.get().to_string(), move |id| {
            let a = id.parse().unwrap_or(0.0);
            o.bg_alpha.set(a);
            s.borrow_mut().conf.set("bg_alpha", a);
            o.win.queue_draw();
        }));

        let layouts = [("vertical", "Vertical"), ("horizontal", "Horizontal")].map(|(a, b)| (a.to_string(), b.to_string()));
        let current = st.borrow().conf.str("layout").unwrap_or("vertical").to_string();
        let (o, s) = (self.clone(), st.clone());
        menu.append(&radio_menu("Layout", &layouts, &current, move |id| {
            s.borrow_mut().conf.set("layout", id);
            o.rebuild(&s.borrow());
        }));

        let caps: Vec<(String, String)> = CAPS
            .iter()
            .map(|c| (c.to_string(), if *c == "off" { "off".into() } else { format!("{c} GHz") }))
            .collect();
        menu.append(&radio_menu("CPU cap", &caps, &read_cap(), set_cap));

        add("Open game log", Rc::new(|| {
            let _ = Command::new("xdg-open").arg(log_path()).spawn();
        }));
        add("Quit", on_quit);
        menu.show_all();
        menu
    }

    /// Re-lays out the grid; needed after pin or layout changes.
    pub fn rebuild(&self, st: &State) {
        for c in self.grid.children() {
            self.grid.remove(&c);
        }
        let horiz = st.conf.str("layout") == Some("horizontal");
        self.grid.attach(&self.cpu, 0, 0, 2, 1);
        let (fc, fr) = if horiz { (2, 0) } else { (0, 1) };
        self.grid.attach(&self.fps.0, fc, fr, 1, 1);
        self.grid.attach(&self.fps.1, fc + 1, fr, 1, 1);
        let pinned: Vec<usize> = (0..st.sensors.len()).filter(|&i| st.pins.contains(&st.sensors[i].key)).collect();
        let mut rows = Vec::new();
        for (n, &i) in pinned.iter().enumerate() {
            let s = &st.sensors[i];
            let shared = pinned.iter().filter(|&&j| st.sensors[j].group == s.group).count() > 1;
            let name = if s.group == "Fans" {
                s.label.clone()
            } else if shared {
                format!("{} {}", s.group, s.label)
            } else {
                s.group.clone()
            };
            let k = n as i32 + 2;
            let (col, row) = if horiz { (2 * k, 0) } else { (0, k) };
            let (l, v) = (label(0.0), label(1.0));
            l.set_markup(&markup(&name, DIM, false));
            self.grid.attach(&l, col, row, 1, 1);
            self.grid.attach(&v, col + 1, row, 1, 1);
            rows.push((i, v));
        }
        *self.rows.borrow_mut() = rows;
        self.grid.show_all();
        self.update(st);
        self.win.resize(1, 1);
    }

    pub fn update(&self, st: &State) {
        let cpu = format!("CPU {:4.1}%  {:.2} GHz", st.cpu.usage, st.cpu.ghz);
        self.cpu.set_markup(&markup(&cpu, DIM, false));
        let (name, val) = &self.fps;
        let focused = st.fps.focused();
        match (st.fps.live, focused) {
            (true, Some(w)) => {
                let n: String = w.name().chars().take(18).collect();
                name.set_markup(&markup(&n, DIM, false));
                val.set_markup(&markup(&format!("{:.0} fps", w.fps), FG, true));
            }
            (false, _) => {
                name.set_markup(&markup("fps", DIM, false));
                val.set_markup(&markup("probe off", DIM, false));
            }
            _ => {}
        }
        let show = !st.fps.live || focused.is_some();
        name.set_visible(show);
        val.set_visible(show);
        for (i, v) in self.rows.borrow().iter() {
            let s = &st.sensors[*i];
            v.set_markup(&markup(&s.text(), s.color(), true));
        }
    }
}
