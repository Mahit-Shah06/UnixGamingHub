use crate::sensors::{read_num, read_str};
use std::fs;
use std::path::Path;

pub struct CpuStats {
    prev: (u64, u64),
    pub usage: f64,
    pub ghz: f64,
    pub load: String,
    pub ac: Option<bool>,
}

fn times() -> (u64, u64) {
    let stat = fs::read_to_string("/proc/stat").unwrap_or_default();
    let v: Vec<u64> = stat
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    if v.len() < 5 {
        return (0, 0);
    }
    (v[3] + v[4], v.iter().sum())
}

impl CpuStats {
    pub fn new() -> Self {
        CpuStats { prev: times(), usage: 0.0, ghz: 0.0, load: String::new(), ac: None }
    }

    pub fn poll(&mut self) {
        let (idle, total) = times();
        let (di, dt) = (idle.saturating_sub(self.prev.0), total.saturating_sub(self.prev.1));
        self.prev = (idle, total);
        self.usage = if dt > 0 { 100.0 * (1.0 - di as f64 / dt as f64) } else { 0.0 };

        let freqs: Vec<i64> = fs::read_dir("/sys/devices/system/cpu")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.strip_prefix("cpu").is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
            })
            .filter_map(|e| read_num(&e.path().join("cpufreq/scaling_cur_freq")))
            .filter(|f| *f > 0)
            .collect();
        self.ghz = if freqs.is_empty() { 0.0 } else { freqs.iter().sum::<i64>() as f64 / freqs.len() as f64 / 1e6 };

        let loadavg = read_str(Path::new("/proc/loadavg"));
        self.load = loadavg.split_whitespace().take(3).collect::<Vec<_>>().join(" ");

        let mains: Vec<String> = fs::read_dir("/sys/class/power_supply")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| read_str(&e.path().join("type")) == "Mains")
            .map(|e| read_str(&e.path().join("online")))
            .collect();
        self.ac = if mains.is_empty() { None } else { Some(mains.iter().any(|m| m == "1")) };
    }

    pub fn summary(&self) -> String {
        let power = match self.ac {
            None => "",
            Some(true) => "  ·  AC",
            Some(false) => "  ·  battery",
        };
        format!("CPU {:4.1}%  ·  {:.2} GHz avg  ·  load {}{power}", self.usage, self.ghz, self.load)
    }
}
