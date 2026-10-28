use gtk::glib;

pub const BG: &str = "#14161a";
pub const FG: &str = "#e6e6e6";
pub const DIM: &str = "#7d8590";
pub const OK: &str = "#4ade80";
pub const WARN: &str = "#fbbf24";
pub const HOT: &str = "#f87171";
pub const NEUTRAL: &str = "#60a5fa";

pub const CSS: &str = r#"
.tm-main { background-color: #14161a; color: #e6e6e6; }
.tm-main label { font-family: "DejaVu Sans Mono"; font-size: 10pt; }
.tm-panel { background-color: #1d2026; border-radius: 6px; }
.tm-head { font-family: "DejaVu Sans"; font-size: 11pt; font-weight: bold; }
.tm-overlay { background-color: transparent; }
.tm-overlay label { font-family: "DejaVu Sans Mono"; font-size: 10pt;
                    text-shadow: 1px 1px 2px #000, 0 0 3px #000; }
"#;

pub fn markup(text: &str, color: &str, bold: bool) -> String {
    let t = glib::markup_escape_text(text);
    if bold {
        format!("<span foreground=\"{color}\"><b>{t}</b></span>")
    } else {
        format!("<span foreground=\"{color}\">{t}</span>")
    }
}

pub fn rgb(hex: &str) -> (f64, f64, f64) {
    let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
    (c(1), c(3), c(5))
}
