use crate::theme::{DIM, HOT, NEUTRAL, OK, WARN};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

pub const HISTORY: usize = 120;

const GROUPS: &[(&str, &str)] = &[
    ("k10temp", "CPU"), ("coretemp", "CPU"), ("nvme", "SSD"), ("spd5118", "RAM"),
    ("mt7921_phy0", "Wi-Fi"), ("acpitz", "ACPI"), ("hp", "Fans"),
];
const ORDER: &[&str] = &["CPU", "iGPU", "dGPU", "SSD", "RAM", "Fans", "Wi-Fi", "ACPI"];

pub fn read_num(p: &Path) -> Option<i64> {
    fs::read_to_string(p).ok()?.trim().parse().ok()
}

pub fn read_str(p: &Path) -> String {
    fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Temp,
    Fan,
    Power,
}

impl Kind {
    fn scale(self) -> f64 {
        match self {
            Kind::Temp => 1000.0,
            Kind::Fan => 1.0,
            Kind::Power => 1e6,
        }
    }
}

pub struct Sensor {
    pub key: String,
    pub group: String,
    pub label: String,
    path: PathBuf,
    pub kind: Kind,
    pub crit: f64,
    pub warn: f64,
    // dGPU runtime_status: reading a suspended card returns EBUSY (or wakes it), so skip it
    gate: Option<PathBuf>,
    pub hist: VecDeque<f64>,
    pub value: Option<f64>,
    pub lo: Option<f64>,
    pub hi: Option<f64>,
    asleep: bool,
}

impl Sensor {
    pub fn poll(&mut self) {
        self.asleep = self.gate.as_deref().is_some_and(|g| read_str(g) == "suspended");
        let raw = if self.asleep { None } else { read_num(&self.path) };
        self.value = raw.map(|r| r as f64 / self.kind.scale());
        if let Some(v) = self.value {
            if self.hist.len() == HISTORY {
                self.hist.pop_front();
            }
            self.hist.push_back(v);
            self.lo = Some(self.lo.map_or(v, |lo| lo.min(v)));
            self.hi = Some(self.hi.map_or(v, |hi| hi.max(v)));
        }
    }

    pub fn color(&self) -> &'static str {
        match self.value {
            None => DIM,
            Some(_) if self.kind != Kind::Temp => NEUTRAL,
            Some(v) if v >= self.crit => HOT,
            Some(v) if v >= self.warn => WARN,
            Some(_) => OK,
        }
    }

    pub fn fmt(&self, v: f64) -> String {
        match self.kind {
            Kind::Temp => format!("{v:.1}°C"),
            Kind::Fan => format!("{v:.0} rpm"),
            Kind::Power => format!("{v:.1} W"),
        }
    }

    pub fn text(&self) -> String {
        if self.asleep {
            return "asleep".into();
        }
        self.value.map_or_else(|| "n/a".into(), |v| self.fmt(v))
    }
}

fn is_dgpu(dev: &Path) -> bool {
    // not boot_vga: that flips to the dGPU when the MUX is set to discrete
    read_num(&dev.join("mem_info_vram_total")).unwrap_or(0) >= 4 << 30
}

fn files(dir: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n.strip_prefix(prefix)
                    .and_then(|r| r.strip_suffix(suffix))
                    .is_some_and(|mid| !mid.is_empty() && mid.bytes().all(|b| b.is_ascii_digit()))
            })
        })
        .collect();
    out.sort();
    out
}

pub fn discover() -> Vec<Sensor> {
    let mut hws = files(Path::new("/sys/class/hwmon"), "hwmon", "");
    hws.sort_by_key(|p| {
        p.file_name().and_then(|n| n.to_str()).and_then(|n| n[5..].parse::<u32>().ok()).unwrap_or(0)
    });
    let lookup = |n: &str| GROUPS.iter().find(|(k, _)| *k == n).map(|(_, g)| g.to_string());
    let (mut sensors, mut counts) = (Vec::new(), HashMap::<String, usize>::new());
    for hw in hws {
        let (name, dev) = (read_str(&hw.join("name")), hw.join("device"));
        let group = if name == "amdgpu" {
            (if is_dgpu(&dev) { "dGPU" } else { "iGPU" }).to_string()
        } else {
            lookup(&name)
                .or_else(|| name.rsplit_once('_').and_then(|(base, _)| lookup(base)))
                .unwrap_or_else(|| name.clone())
        };
        let n = {
            let c = counts.entry(group.clone()).or_default();
            *c += 1;
            *c
        };
        let devid = fs::canonicalize(&dev)
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_else(|| hw.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default());
        let gate = (group == "dGPU").then(|| dev.join("power/runtime_status"));
        let kinds = [
            (Kind::Temp, "temp", "_input"), (Kind::Fan, "fan", "_input"),
            (Kind::Power, "power", "_average"), (Kind::Power, "power", "_input"),
        ];
        for (kind, prefix, suffix) in kinds {
            for f in files(&hw, prefix, suffix) {
                let fname = f.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let stem = fname.rsplit_once('_').map_or(fname.as_str(), |(s, _)| s).to_string();
                let mut label = read_str(&hw.join(format!("{stem}_label")));
                if label.is_empty() {
                    label = match kind {
                        Kind::Fan => format!("Fan {}", &stem[3..]),
                        _ if group == "RAM" => format!("DIMM {n}"),
                        _ if group == "ACPI" => format!("zone {}", n - 1),
                        Kind::Temp => "temp".into(),
                        Kind::Power => "power".into(),
                    };
                }
                let crit = read_num(&hw.join(format!("{stem}_crit")))
                    .filter(|c| 40_000 < *c && *c < 150_000)
                    .map_or(95.0, |c| c as f64 / 1000.0);
                sensors.push(Sensor {
                    key: format!("{name}@{devid}/{stem}"),
                    group: group.clone(),
                    label,
                    path: f,
                    kind,
                    crit,
                    warn: crit - 15.0,
                    gate: gate.clone(),
                    hist: VecDeque::with_capacity(HISTORY),
                    value: None,
                    lo: None,
                    hi: None,
                    asleep: false,
                });
            }
        }
    }
    sensors.sort_by_key(|s| ORDER.iter().position(|g| *g == s.group).unwrap_or(ORDER.len()));
    sensors
}

pub fn default_pins(sensors: &[Sensor]) -> Vec<String> {
    let mut seen = Vec::new();
    let mut pins = Vec::new();
    for s in sensors {
        let g = s.group.as_str();
        if ["CPU", "iGPU", "dGPU", "SSD"].contains(&g) && s.kind == Kind::Temp && !seen.contains(&g) {
            seen.push(g);
            pins.push(s.key.clone());
        }
    }
    pins.extend(sensors.iter().filter(|s| s.group == "Fans").map(|s| s.key.clone()));
    pins
}

/// gpu_busy_percent per card; the dGPU is skipped while it sleeps so polling never wakes it.
pub struct Gpus {
    cards: Vec<(bool, PathBuf)>,
}

#[derive(Default, Clone, Copy)]
pub struct GpuLoad {
    pub dgpu: Option<f64>,
    pub igpu: Option<f64>,
}

impl Gpus {
    pub fn discover() -> Self {
        let cards = files(Path::new("/sys/class/drm"), "card", "")
            .into_iter()
            .map(|p| p.join("device"))
            .filter(|dev| dev.join("gpu_busy_percent").exists())
            .map(|dev| (is_dgpu(&dev), dev))
            .collect();
        Gpus { cards }
    }

    pub fn load(&self) -> GpuLoad {
        let mut g = GpuLoad::default();
        for (dgpu, dev) in &self.cards {
            if *dgpu && read_str(&dev.join("power/runtime_status")) == "suspended" {
                continue;
            }
            let busy = read_num(&dev.join("gpu_busy_percent")).map(|v| v as f64);
            if *dgpu { g.dgpu = busy } else { g.igpu = busy }
        }
        g
    }
}
