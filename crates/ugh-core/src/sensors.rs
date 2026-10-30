use std::fs;
use std::path::{Path, PathBuf};

pub enum readMethod {
    File {scale: f64},
}

pub struct Sensor {
    pub label: String,
    pub device: String,
    pub path: PathBuf,
    pub Method: readMethod,
    pub unit: &'static str,
}

fn read_sensors(path: &Path) -> Option<f64>{
    fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()
}

impl Sensor {
    pub fn read(&self) -> Option<f64> {
        match &self.Method {
            readMethod::File{scale} => Some(read_sensors(&self.path)? / scale),
        }
    }
}
