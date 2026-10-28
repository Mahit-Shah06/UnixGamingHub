use crate::conf::parse_geom;
use crate::gamelog::read_cap;
use crate::sensors::{Kind, Sensor, HISTORY};
use crate::theme::{markup, rgb, BG, DIM, FG};
use crate::{set_cap, Shared, State, CAPS};
use gtk::prelude::*;
use gtk::{cairo, glib};
use std::rc::Rc;

const SPARK_W: i32 = 160;
const SPARK_H: i32 = 20;

pub struct MainWin {
    pub win: gtk::Window,
    summary: gtk::Label,
    hottest: gtk::Label,
    fps: gtk::Label,
    rows: Vec<(gtk::Label, gtk::DrawingArea, gtk::Label)>,
}

fn label(text: &str, color: &str, xalign: f32) -> gtk::Label {
    let l = gtk::Label::new(None);
    l.set_markup(&markup(text, color, false));
    l.set_xalign(xalign);
    l
}

fn draw_spark(cr: &cairo::Context, s: &Sensor) {
    let (r, g, b) = rgb(BG);
    cr.set_source_rgb(r, g, b);
    let _ = cr.paint();
    if s.hist.len() < 2 {
        return;
    }
    let lo = s.hist.iter().copied().fold(f64::MAX, f64::min);
    let hi = s.hist.iter().copied().fold(f64::MIN, f64::max);
    let min_span = if s.kind == Kind::Temp { 5.0 } else { (hi * 0.1).max(1.0) };
    let span = (hi - lo).max(min_span);
    let lo = (hi + lo) / 2.0 - span / 2.0;
    let step = SPARK_W as f64 / (HISTORY - 1) as f64;
    let x0 = SPARK_W as f64 - step * (s.hist.len() - 1) as f64;
    for (i, v) in s.hist.iter().enumerate() {
        let (x, y) = (x0 + i as f64 * step, SPARK_H as f64 - 3.0 - (v - lo) / span * (SPARK_H as f64 - 6.0));
        if i == 0 { cr.move_to(x, y) } else { cr.line_to(x, y) }
    }
    let (r, g, b) = rgb(s.color());
    cr.set_source_rgb(r, g, b);
    cr.set_line_width(1.5);
    let _ = cr.stroke();
}

impl MainWin {
    pub fn new(st: &Shared, on_overlay: Rc<dyn Fn()>, on_quit: Rc<dyn Fn()>) -> Self {
        let win = gtk::Window::new(gtk::WindowType::Toplevel);
        win.set_title("tempmon");
        win.style_context().add_class("tm-main");
        if let Some(((w, h), pos)) = st.borrow().conf.str("main_geom").and_then(parse_geom) {
            win.resize(w, h);
            if let Some((x, y)) = pos {
                win.move_(x, y);
            }
        }
        let q = on_quit.clone();
        win.connect_delete_event(move |_, _| {
            q();
            glib::Propagation::Stop
        });
        let o = on_overlay.clone();
        win.connect_key_press_event(move |_, e| {
            if e.state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                match e.keyval().to_unicode() {
                    Some('o') => o(),
                    Some('q') => on_quit(),
                    _ => return glib::Propagation::Proceed,
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });

        let outer = gtk::Box::new(gtk::Orientation::Vertical, 4);
        win.add(&outer);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.set_margin_start(12);
        top.set_margin_end(12);
        top.set_margin_top(10);
        let summary = label("", FG, 0.0);
        top.pack_start(&summary, true, true, 0);
        let button = gtk::Button::with_label("Overlay  ⧉");
        button.connect_clicked(move |_| on_overlay());
        top.pack_end(&button, false, false, 0);
        let cap = gtk::ComboBoxText::new();
        for c in CAPS {
            cap.append(Some(c), c);
        }
        cap.set_active_id(Some(&read_cap()));
        cap.connect_changed(|c| {
            if let Some(id) = c.active_id() {
                set_cap(&id);
            }
        });
        top.pack_end(&cap, false, false, 0);
        top.pack_end(&label("cap GHz", DIM, 1.0), false, false, 0);
        outer.pack_start(&top, false, false, 0);

        let hottest = label("", DIM, 0.0);
        let fps = label("", DIM, 0.0);
        for l in [&hottest, &fps] {
            l.set_margin_start(12);
            outer.pack_start(l, false, false, 0);
        }

        let body = gtk::Grid::new();
        body.style_context().add_class("tm-panel");
        body.set_column_spacing(12);
        body.set_row_spacing(1);
        body.set_margin_start(12);
        body.set_margin_end(12);
        body.set_margin_bottom(12);
        body.set_margin_top(6);
        outer.pack_start(&body, true, true, 0);
        let headers = ["sensor", "now", &format!("last {HISTORY}s"), "min–max", "overlay"];
        for (col, h) in headers.iter().enumerate() {
            let l = label(h, DIM, 0.0);
            l.set_margin_top(8);
            l.set_margin_start(if col == 0 { 8 } else { 0 });
            body.attach(&l, col as i32, 0, 1, 1);
        }

        let mut rows = Vec::new();
        let (mut row, mut group) = (1, String::new());
        let s = st.borrow();
        for (i, sensor) in s.sensors.iter().enumerate() {
            if sensor.group != group {
                group = sensor.group.clone();
                let g = label(&group, FG, 0.0);
                g.style_context().add_class("tm-head");
                g.set_margin_top(8);
                g.set_margin_start(8);
                body.attach(&g, 0, row, 1, 1);
                row += 1;
            }
            let name = label(&format!("  {}", sensor.label), FG, 0.0);
            name.set_margin_start(8);
            body.attach(&name, 0, row, 1, 1);
            let val = label("", FG, 1.0);
            val.set_width_chars(10);
            body.attach(&val, 1, row, 1, 1);
            let area = gtk::DrawingArea::new();
            area.set_size_request(SPARK_W, SPARK_H);
            let shared = st.clone();
            area.connect_draw(move |_, cr| {
                if let Ok(s) = shared.try_borrow() {
                    draw_spark(cr, &s.sensors[i]);
                }
                glib::Propagation::Stop
            });
            body.attach(&area, 2, row, 1, 1);
            let mm = label("", DIM, 0.0);
            mm.set_width_chars(18);
            body.attach(&mm, 3, row, 1, 1);
            let pin = gtk::CheckButton::new();
            pin.set_active(s.pins.contains(&sensor.key));
            pin.set_halign(gtk::Align::Center);
            let (shared, key) = (st.clone(), sensor.key.clone());
            pin.connect_toggled(move |c| shared.borrow_mut().toggle_pin(&key, c.is_active()));
            body.attach(&pin, 4, row, 1, 1);
            rows.push((val, area, mm));
            row += 1;
        }
        drop(s);
        MainWin { win, summary, hottest, fps, rows }
    }

    pub fn update(&self, st: &State) {
        self.summary.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&st.cpu.summary())));
        let hot = st
            .sensors
            .iter()
            .filter(|s| s.kind == Kind::Temp && s.value.is_some())
            .max_by(|a, b| (a.value.unwrap_or(0.0) / a.crit).total_cmp(&(b.value.unwrap_or(0.0) / b.crit)));
        if let Some(h) = hot {
            let text = format!(
                "closest to limit: {} {} {}  (warn {:.0}° / crit {:.0}°)",
                h.group, h.label, h.text(), h.warn, h.crit
            );
            self.hottest.set_markup(&markup(&text, h.color(), false));
        }
        let fps = if !st.fps.live {
            "fps: probe off. Log out and back in once to load the FPS Probe extension.".to_string()
        } else {
            let focused = st.fps.focused().map_or("nothing focused".into(), |w| format!("{} {:.0} fps", w.name(), w.fps));
            let others: Vec<String> = st
                .fps
                .windows
                .iter()
                .filter(|w| !w.focused && w.fps >= 1.0)
                .take(3)
                .map(|w| format!("{} {:.0}", w.name(), w.fps))
                .collect();
            let also = if others.is_empty() { String::new() } else { format!("   ·   also drawing: {}", others.join(", ")) };
            format!("focused: {focused}{also}")
        };
        self.fps.set_markup(&markup(&fps, if st.fps.live { FG } else { DIM }, false));
        for (s, (val, area, mm)) in st.sensors.iter().zip(&self.rows) {
            val.set_markup(&markup(&s.text(), s.color(), true));
            let range = match (s.lo, s.hi) {
                (Some(lo), Some(hi)) if s.kind == Kind::Fan => format!("{lo:.0} – {hi:.0}"),
                (Some(lo), Some(hi)) => format!("{lo:.1} – {hi:.1}"),
                _ => String::new(),
            };
            mm.set_markup(&markup(&range, DIM, false));
            area.queue_draw();
        }
    }
}
