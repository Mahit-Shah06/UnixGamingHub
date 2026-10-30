use std::fs;
use crate::sensors::Sensor;
use crate::names::{friendly_name, friendly_label, name_gpus, number_drives, device_rank};
use crate::paths::HWMON;

const TYPES: [(&str, f64, &str); 5] = [
    ("temp", 1000.0, "°C"),
    ("power", 1_000_000.0, "W"),
    ("freq", 1_000_000.0, "MHz"),
    ("fan", 1.0, "RPM"),
    ("in", 1000.0, "V"),
];

pub fn find_sensors() -> Option<Vec<Sensor>> {
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

fn type_rank(unit: &str) -> usize { 
    for (i, tup) in TYPES.iter().enumerate() { 
        if tup.2 == unit { 
            return i; 
        } 
    } 
    99 
}
