use std::fs;
use std::path::{Path, PathBuf};
use crate::names::{friendly_name, friendly_label, name_gpus, number_drives, device_rank};
use crate::paths::HWMON;

const TYPES: [(&str, f64, &str); 5] = [
    ("temp", 1000.0, "°C"),
    ("power", 1_000_000.0, "W"),
    ("freq", 1_000_000.0, "MHz"),
    ("fan", 1.0, "RPM"),
    ("in", 1000.0, "V"),
];

pub struct Sensor {
    pub label: String,
    pub device: String,
    pub path: PathBuf,
    pub scale: f64,
    pub unit: &'static str,
}

pub fn sensor_names() -> Option<Vec<Sensor>> {
    let mut sensors = Vec::new();

    for entry in fs::read_dir(HWMON).ok()? {
        let Ok(entry) = entry else {continue};
        let folder = entry.path();

        let device = friendly_name(fs::read_to_string(folder.join("name")).ok()?.trim());

        for file in fs::read_dir(&folder).ok()? {
            let path = file.ok()?.path();
            let file_name = path.file_name()?.to_string_lossy().to_string();

            let base = file_name.split('_').next()?;
            let label = match fs::read_to_string(folder.join(format!("{base}_label"))){
                Ok(text) => friendly_label(text.trim()),
                Err(_) => base.to_string(),
            };

            if file_name.ends_with("_input") || file_name.ends_with("_average") {
                for (prefix, scale, unit) in TYPES {
                    if file_name.starts_with(prefix) {
                        sensors.push(Sensor 
                            {label: label.clone(),
                            device: device.clone(),
                            path: path.clone(),
                            scale, unit }
                        );
                        break;
                    }
                }
            }
        }
    }

    name_gpus(&mut sensors);
    number_drives(&mut sensors);
    sensors.sort_by_key(|s| (device_rank(&s.device), s.device.clone(), type_rank(s.unit), s.path.clone()));
    Some(sensors)
}

// A type's rank is its row in TYPES, so the row order there is the order sensors show up in.
fn type_rank(unit: &str) -> usize {
    for (i, tup) in TYPES.iter().enumerate() {
        if tup.2 == unit {
            return i;
        }
    }
    99
}

fn read_sensors(path: &Path) -> Option<f64>{
    fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()
}

impl Sensor {
    pub fn read(&self) -> Option<f64> {
        Some(read_sensors(&self.path)? / self.scale)
    }
}
