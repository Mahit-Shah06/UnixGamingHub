use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// ~/.config/tempmon.json, same keys the Python version used so settings carry over.
pub struct Conf {
    map: Map<String, Value>,
    path: PathBuf,
}

impl Conf {
    pub fn load() -> Self {
        let path = home().join(".config/tempmon.json");
        let map = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        Conf { map, path }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.map.get(key)?.as_str()
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.map.get(key)?.as_f64()
    }

    pub fn strings(&self, key: &str) -> Option<Vec<String>> {
        let arr = self.map.get(key)?.as_array()?;
        Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    }

    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.map.insert(key.to_string(), value.into());
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(&self.map) {
            let _ = fs::write(&self.path, s);
        }
    }
}

/// "+x+y" (x/y may be negative)
pub fn parse_pos(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.strip_prefix('+')?.split_once('+')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// "WxH+x+y"
pub fn parse_geom(s: &str) -> Option<((i32, i32), Option<(i32, i32)>)> {
    let (size, pos) = match s.split_once('+') {
        Some((size, rest)) => (size, parse_pos(&format!("+{rest}"))),
        None => (s, None),
    };
    let (w, h) = size.split_once('x')?;
    Some(((w.parse().ok()?, h.parse().ok()?), pos))
}
