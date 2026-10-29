use std::fs;
use std::path::PathBuf;
use crate::sensors::Sensor;

// hwmon folder name prefix → friendly name.
// The first match wins, so specific prefixes go above general ones (asus_wmi_sensors above asus).
const DEVICES: &[(&str, &str)] = &[
    // CPU
    ("k10temp", "CPU"),          // AMD
    ("zenpower", "CPU"),         // AMD, alternative driver
    ("coretemp", "CPU"),         // Intel
    ("cpu_thermal", "CPU"),      // ARM boards, e.g. Raspberry Pi
    // GPU
    ("amdgpu", "GPU"),           // AMD
    ("radeon", "GPU"),           // older AMD
    ("i915", "GPU"),             // Intel
    ("xe", "GPU"),               // newer Intel (Arc, Lunar Lake+)
    ("nouveau", "GPU"),          // NVIDIA, open-source driver
    // Storage
    ("nvme", "SSD"),
    ("drivetemp", "Disk"),       // SATA SSDs and hard drives
    // Memory
    ("spd5118", "RAM"),          // DDR5
    ("jc42", "RAM"),             // DDR4
    // Motherboard
    ("acpitz", "Board"),
    ("nct", "Board"),            // Nuvoton chips on desktop boards (also have fans)
    ("it87", "Board"),           // ITE chips on desktop boards (also have fans)
    ("asus_wmi_sensors", "Board"),
    ("asusec", "Board"),
    // Laptop fans
    ("hp", "Fans"),
    ("thinkpad", "Fans"),
    ("dell_smm", "Fans"),
    ("asus", "Fans"),
    ("applesmc", "Fans"),
    // Network
    ("iwlwifi", "Wi-Fi"),        // Intel
    ("mt7", "Wi-Fi"),            // MediaTek
    ("ath", "Wi-Fi"),            // Qualcomm Atheros
    ("rtw", "Wi-Fi"),            // Realtek
    ("r8169", "Ethernet"),       // Realtek
    // Power
    ("BAT", "Battery"),
    ("ACAD", "Charger"),
    ("ADP", "Charger"),
    ("AC", "Charger"),
    ("ucsi", "USB-C"),
];

// Sensor label from the driver → friendly label. Exact matches; labels not listed stay as they are.
const LABELS: &[(&str, &str)] = &[
    // CPU
    ("Tctl", "temp"),            // AMD
    ("Tdie", "die temp"),        // AMD, older chips
    ("Package id 0", "temp"),    // Intel
    // GPU (AMD)
    ("edge", "temp"),
    ("junction", "hotspot temp"),
    ("mem", "memory temp"),
    ("PPT", "power"),
    ("sclk", "core clock"),
    ("mclk", "memory clock"),
    ("vddgfx", "core voltage"),
    ("vddnb", "SoC voltage"),
    // SSD
    ("Composite", "temp"),
];

// With 2+ GPUs, the one with the least video memory is the built-in one (iGPU), the rest are dGPUs.
// With only one GPU it stays "GPU". A GPU without a VRAM file (e.g. Intel built-in) counts as 0.
pub fn name_gpus(sensors: &mut [Sensor]) {
    let mut gpus: Vec<(PathBuf, u64)> = Vec::new();
    for s in sensors.iter() {
        let Some(folder) = s.path.parent() else { continue };
        if s.device == "GPU" && !gpus.iter().any(|(f, _)| f == folder) {
            let vram = fs::read_to_string(folder.join("device/mem_info_vram_total"))
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(0);
            gpus.push((folder.to_path_buf(), vram));
        }
    }
    if gpus.len() < 2 {
        return;
    }
    let Some((igpu, _)) = gpus.iter().min_by_key(|(_, vram)| *vram) else { return };
    for s in sensors.iter_mut() {
        if s.device == "GPU" {
            s.device = if s.path.parent() == Some(igpu.as_path()) { "iGPU" } else { "dGPU" }.to_string();
        }
    }
}

// With 2+ drives or memory sticks of the same kind, number them (SSD 1, SSD 2), ordered by where they
// sit in the system so the numbers stay the same between runs. One physical drive = one number,
// however many partitions it has, because hwmon has one folder per drive, not per partition.
pub fn number_drives(sensors: &mut [Sensor]) {
    for kind in ["SSD", "Disk", "RAM"] {
        let mut folders: Vec<PathBuf> = Vec::new();
        for s in sensors.iter() {
            let Some(folder) = s.path.parent() else { continue };
            if s.device == kind && !folders.iter().any(|f| f == folder) {
                folders.push(folder.to_path_buf());
            }
        }
        if folders.len() < 2 {
            continue;
        }
        folders.sort_by_key(|f| fs::canonicalize(f.join("device")).unwrap_or_default());
        for s in sensors.iter_mut() {
            if s.device != kind {
                continue;
            }
            if let Some(i) = folders.iter().position(|f| Some(f.as_path()) == s.path.parent()) {
                s.device = format!("{kind} {}", i + 1);
            }
        }
    }
}

pub fn friendly_name(device: &str) -> String {
    for tup in DEVICES {
        if device.starts_with(tup.0) {
            return  tup.1.to_string();
        }
    }
    device.to_string()
}

pub fn friendly_label(label: &str) -> String {
    for tup in LABELS {
        if label == tup.0 {
            return tup.1.to_string();
        }
    }
    label.to_string()
}
