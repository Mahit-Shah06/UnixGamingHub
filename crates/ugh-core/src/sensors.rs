use std::fs;
use std::path::{Path, PathBuf};

pub struct Sensor {
    pub label: String,
    pub device: String,
    pub path: PathBuf,
    pub scale: f64,
    pub unit: &'static str,
}

fn read_sensors(path: &Path) -> Option<f64>{
    fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()
}

impl Sensor {
    pub fn read(&self) -> Option<f64> {
        Some(read_sensors(&self.path)? / self.scale)
    }
}
