//! App session logger. Rides the main 1 s tick and the probe's numbers, so it adds no polling.
//! A session is the focused app drawing at >= ACTIVE_FPS; it's only written to the log once it
//! has lasted ANNOUNCE_S, so alt-tabbing through a browser doesn't spam it.
use crate::conf::home;
use crate::cpu::CpuStats;
use crate::fps::FpsProbe;
use crate::sensors::{GpuLoad, Kind, Sensor};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

const ACTIVE_FPS: f64 = 10.0;
const ANNOUNCE_S: usize = 30;
const SNAPSHOT_S: u64 = 30;
const END_AFTER_S: u64 = 10;
const MAX_LOG: u64 = 1 << 20;

pub fn log_path() -> PathBuf {
    home().join(".local/share/tempmon/games.log")
}

pub fn read_cap() -> String {
    fs::read_to_string("/etc/cpu-cap.conf").map(|s| s.trim().to_string()).unwrap_or_else(|_| "off".into())
}

fn clock(fmt: &str) -> String {
    Command::new("date")
        .arg(format!("+{fmt}"))
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

struct Sample {
    fps: f64,
    gpu: f64,
    on_igpu: bool,
    cpu_t: f64,
    gpu_t: f64,
}

struct Session {
    app: String,
    pid: i64,
    started: String,
    start: Instant,
    last_active: Instant,
    last_snap: Instant,
    samples: Vec<Sample>,
    battery: bool,
}

pub struct GameLog {
    s: Option<Session>,
}

fn temp_of(sensors: &[Sensor], group: &str) -> Option<f64> {
    sensors.iter().find(|s| s.group == group && s.kind == Kind::Temp)?.value
}

impl GameLog {
    pub fn new() -> Self {
        let path = log_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG) {
            if let Ok(text) = fs::read_to_string(&path) {
                let keep = text.len() - (MAX_LOG / 2) as usize;
                let cut = text[keep..].find('\n').map_or(keep, |i| keep + i + 1);
                let _ = fs::write(&path, &text[cut..]);
            }
        }
        GameLog { s: None }
    }

    pub fn update(&mut self, fps: &FpsProbe, cpu: &CpuStats, sensors: &[Sensor], gpu: GpuLoad) {
        let active = fps.focused().filter(|w| w.fps >= ACTIVE_FPS);
        let same = |s: &Session| active.is_some_and(|w| w.pid == s.pid && w.name() == s.app);
        if self.s.as_ref().is_some_and(|s| active.is_some() && !same(s)) {
            self.end();
        }
        let Some(w) = active else {
            if self.s.as_ref().is_some_and(|s| s.last_active.elapsed().as_secs() >= END_AFTER_S) {
                self.end();
            }
            return;
        };
        let now = Instant::now();
        let s = self.s.get_or_insert_with(|| Session {
            app: w.name().to_string(),
            pid: w.pid,
            started: clock("%Y-%m-%d %H:%M:%S"),
            start: now,
            last_active: now,
            last_snap: now,
            samples: Vec::new(),
            battery: false,
        });
        // dGPU asleep (None) while the app draws means it's rendering on the iGPU
        let on_igpu = gpu.dgpu.map_or(true, |d| d < 5.0) && gpu.igpu.unwrap_or(0.0) > 30.0;
        let sample = Sample {
            fps: w.fps,
            gpu: if on_igpu { gpu.igpu.unwrap_or(0.0) } else { gpu.dgpu.unwrap_or(0.0) },
            on_igpu,
            cpu_t: temp_of(sensors, "CPU").unwrap_or(0.0),
            gpu_t: temp_of(sensors, if on_igpu { "iGPU" } else { "dGPU" }).unwrap_or(0.0),
        };
        s.last_active = now;
        s.battery |= cpu.ac == Some(false);
        s.samples.push(sample);
        let n = s.samples.len();
        if n == ANNOUNCE_S {
            let line = format!(
                "\n{}  {} (pid {}) started  (cap {} GHz, {})",
                s.started, s.app, s.pid, read_cap(),
                if cpu.ac == Some(false) { "battery" } else { "AC" }
            );
            write(&line);
        }
        if n >= ANNOUNCE_S && s.last_snap.elapsed().as_secs() >= SNAPSHOT_S {
            s.last_snap = now;
            let x = &s.samples[n - 1];
            write(&format!(
                "{}  fps {:.0}  cpu {:.0}% {:.0}°C  gpu {:.0}% {:.0}°C{}",
                clock("%H:%M:%S"), x.fps, cpu.usage, x.cpu_t, x.gpu, x.gpu_t,
                if x.on_igpu { " (iGPU)" } else { "" }
            ));
        }
    }

    pub fn end(&mut self) {
        let Some(s) = self.s.take() else { return };
        if s.samples.len() < ANNOUNCE_S {
            return;
        }
        let mut fps: Vec<f64> = s.samples.iter().map(|x| x.fps).collect();
        fps.sort_by(f64::total_cmp);
        let n = fps.len() as f64;
        let avg = fps.iter().sum::<f64>() / n;
        let low = fps[fps.len() / 100];
        let sd = (fps.iter().map(|f| (f - avg).powi(2)).sum::<f64>() / n).sqrt();
        let gpu = s.samples.iter().map(|x| x.gpu).sum::<f64>() / n;
        let igpu_share = s.samples.iter().filter(|x| x.on_igpu).count() as f64 / n;
        let cpu_t = s.samples.iter().map(|x| x.cpu_t).fold(0.0, f64::max);
        let gpu_t = s.samples.iter().map(|x| x.gpu_t).fold(0.0, f64::max);
        write(&format!(
            "{}  {} stopped after {:.0} min\n  fps avg {avg:.0}, 1% low {low:.0}, min {:.0}  |  gpu {gpu:.0}% busy, max {gpu_t:.0}°C  |  cpu max {cpu_t:.0}°C",
            clock("%H:%M:%S"), s.app, s.start.elapsed().as_secs_f64() / 60.0, fps[0]
        ));
        for tip in tips(avg, low, sd, gpu, igpu_share, cpu_t, gpu_t, s.battery) {
            write(&format!("  - {tip}"));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tips(avg: f64, low: f64, sd: f64, gpu: f64, igpu_share: f64, cpu_t: f64, gpu_t: f64, battery: bool) -> Vec<String> {
    let (cap, mut out) = (read_cap(), Vec::new());
    if igpu_share > 0.5 {
        out.push("Ran on the iGPU (680M). Use the RX 6650M: Steam launch options `DRI_PRIME=1 %command%`.".into());
    }
    // a flat line means vsync or a limiter, not a bottleneck
    if sd < avg * 0.03 {
        out.push(format!("FPS sat flat at ~{avg:.0}: vsync/limiter is holding it. Lower that limit to cut heat."));
    } else if gpu >= 90.0 {
        out.push(format!("GPU-bound ({gpu:.0}% busy): lower resolution/settings or turn on FSR. Raising the CPU cap won't help."));
    } else if gpu < 75.0 {
        let hint = if cap != "off" { " Try `cpu-cap 3.0` (or `cpu-cap off`) for this one." } else { "" };
        out.push(format!("GPU only {gpu:.0}% busy, so the CPU is the limit. Cap is {cap} GHz.{hint}"));
    }
    if low < avg * 0.5 {
        out.push(format!("Stutter: 1% low ({low:.0}) is under half the average. First runs compile shaders, so it often settles on a second launch; otherwise close background apps."));
    }
    if cpu_t >= 90.0 {
        out.push(format!("CPU peaked at {cpu_t:.0}°C: lower the cap or raise the laptop's rear for airflow."));
    }
    if gpu_t >= 90.0 {
        out.push(format!("GPU peaked at {gpu_t:.0}°C: cap FPS in-game (or `DXVK_FRAME_RATE=60 %command%` for Proton games)."));
    }
    if battery {
        out.push("Ran on battery: the GPU is power-limited unplugged. Plug in.".into());
    }
    if out.is_empty() {
        out.push("No obvious bottleneck.".into());
    }
    out
}

fn write(line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "{line}");
    }
}
