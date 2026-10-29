use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Reads what the fps-probe GNOME Shell extension writes: frames each window
/// presented to the compositor in the last second.
const STALE_MS: f64 = 3000.0;

#[derive(Clone)]
pub struct WinFps {
    pub app: String,
    pub class: String,
    pub title: String,
    pub pid: i64,
    pub fps: f64,
    pub focused: bool,
}

impl WinFps {
    pub fn name(&self) -> &str {
        // Proton games have no .desktop match, so the tracker reports "steam_app_<id>"
        if self.app.starts_with("steam_app_") && !self.title.is_empty() {
            &self.title
        } else if self.app.is_empty() {
            &self.class
        } else {
            &self.app
        }
    }
}

#[derive(Default)]
pub struct FpsProbe {
    pub windows: Vec<WinFps>,
    /// false when the extension isn't running (not enabled yet, or shell not restarted)
    pub live: bool,
}

fn out_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("fps-probe.json")
}

impl FpsProbe {
    pub fn poll(&mut self) {
        self.windows.clear();
        self.live = false;
        let Ok(text) = fs::read_to_string(out_path()) else { return };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { return };
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64);
        if now_ms - v["ts"].as_f64().unwrap_or(0.0) > STALE_MS {
            return;
        }
        self.live = true;
        let s = |w: &Value, k: &str| w[k].as_str().unwrap_or("").to_string();
        for w in v["windows"].as_array().into_iter().flatten() {
            self.windows.push(WinFps {
                app: s(w, "app"),
                class: s(w, "class"),
                title: s(w, "title"),
                pid: w["pid"].as_i64().unwrap_or(0),
                fps: w["fps"].as_f64().unwrap_or(0.0),
                focused: w["focused"].as_bool().unwrap_or(false),
            });
        }
        self.windows.sort_by(|a, b| b.fps.total_cmp(&a.fps));
    }

    pub fn focused(&self) -> Option<&WinFps> {
        self.windows.iter().find(|w| w.focused)
    }
}
