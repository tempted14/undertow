use serde::Serialize;
use std::time::Instant;

// Keep the original target visible while allowing user-approved visual headroom.
const REQUESTED_TARGET_BYTES: u64 = 90_000_000;
const MEMORY_TARGET_BYTES: u64 = 200_000_000;
const MEMORY_BUDGET_BYTES: u64 = 256_000_000;

#[derive(Clone, Default, Serialize)]
pub struct Sample {
    pub seconds: f64,
    pub working_set_bytes: u64,
    pub private_bytes: u64,
    pub cpu_seconds: f64,
    pub frame_cpu_ms: f32,
}
pub struct Metrics {
    pub samples: Vec<Sample>,
    start: Instant,
    last: Instant,
    pub current: Sample,
    last_animation: Option<Instant>,
    animation_intervals_ms: Vec<f64>,
    last_animation_frame: Option<u64>,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            start: Instant::now(),
            last: Instant::now() - std::time::Duration::from_secs(2),
            current: Sample::default(),
            last_animation: None,
            animation_intervals_ms: Vec::new(),
            last_animation_frame: None,
        }
    }
}
impl Metrics {
    pub fn frame(&mut self, animated: bool, frame_nr: u64) {
        if self.last_animation_frame == Some(frame_nr) {
            return;
        }
        self.last_animation_frame = Some(frame_nr);
        let now = Instant::now();
        if animated {
            if let Some(last) = self.last_animation
                && self.animation_intervals_ms.len() < 14400
            {
                self.animation_intervals_ms
                    .push(now.duration_since(last).as_secs_f64() * 1000.);
            }
            self.last_animation = Some(now);
        } else {
            self.last_animation = None;
        }
    }
    pub fn sample(&mut self, frame_cpu: f32, extra: &[u32]) {
        if self.last.elapsed().as_secs_f64() < 1.0 {
            return;
        }
        self.last = Instant::now();
        let mut s = read_process();
        for pid in extra {
            let child = read_child(*pid);
            s.working_set_bytes += child.working_set_bytes;
            s.private_bytes += child.private_bytes;
            s.cpu_seconds += child.cpu_seconds;
        }
        s.seconds = self.start.elapsed().as_secs_f64();
        s.frame_cpu_ms = frame_cpu * 1000.0;
        self.current = s.clone();
        if self.samples.len() < 7200 {
            self.samples.push(s);
        }
    }
    pub fn elapsed(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
    pub fn report(&self, details: serde_json::Value) -> serde_json::Value {
        let mut intervals = self.animation_intervals_ms.clone();
        intervals.sort_by(f64::total_cmp);
        let frames = intervals.len();
        let total = intervals.iter().sum::<f64>();
        let animation = serde_json::json!({"frames_measured":frames,"average_fps":if total>0.{frames as f64*1000./total}else{0.},"median_frame_ms":intervals.get(frames/2),"p95_frame_ms":intervals.get(frames.saturating_mul(95)/100)});
        let peak_ws = self
            .samples
            .iter()
            .map(|s| s.working_set_bytes)
            .max()
            .unwrap_or(0);
        let peak_private = self
            .samples
            .iter()
            .map(|s| s.private_bytes)
            .max()
            .unwrap_or(0);
        let cpu = match (self.samples.first(), self.samples.last()) {
            (Some(a), Some(b)) if b.seconds > a.seconds => {
                (b.cpu_seconds - a.cpu_seconds) / (b.seconds - a.seconds) * 100.0
            }
            _ => 0.0,
        };
        serde_json::json!({"requested_target_bytes":REQUESTED_TARGET_BYTES,"within_requested_target":peak_ws>0&&peak_ws<REQUESTED_TARGET_BYTES&&peak_private<REQUESTED_TARGET_BYTES,"lyric_animation":animation,"app":"Undertow","version":env!("CARGO_PKG_VERSION"),"measurement":"Windows GetProcessMemoryInfo; untrimmed working set and private committed bytes; main process plus the optional native lyrics child; shared pages counted in both working sets. GPU memory is separate and not included.","soft_target_bytes":MEMORY_TARGET_BYTES,"within_original_target":peak_ws>0&&peak_ws<MEMORY_TARGET_BYTES&&peak_private<MEMORY_TARGET_BYTES,"budget_bytes":MEMORY_BUDGET_BYTES,"peak_working_set_bytes":peak_ws,"peak_private_bytes":peak_private,"within_process_memory_budget":peak_ws>0&&peak_ws<MEMORY_BUDGET_BYTES&&peak_private<MEMORY_BUDGET_BYTES,"cpu_percent_one_core":cpu,"duration_seconds":self.elapsed(),"details":details,"samples":self.samples})
    }
}
#[cfg(windows)]
fn read_process() -> Sample {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };
    let mut c = PROCESS_MEMORY_COUNTERS_EX::default();
    c.cb = std::mem::size_of_val(&c) as u32;
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        let p = GetCurrentProcess();
        let _ = GetProcessMemoryInfo(p, &mut c as *mut _ as *mut _, c.cb);
        let _ = GetProcessTimes(p, &mut creation, &mut exit, &mut kernel, &mut user);
    }
    let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
    Sample {
        working_set_bytes: c.WorkingSetSize as u64,
        private_bytes: c.PrivateUsage as u64,
        cpu_seconds: (ticks(kernel) + ticks(user)) as f64 / 10_000_000.0,
        ..Default::default()
    }
}
#[cfg(not(windows))]
fn read_process() -> Sample {
    Sample::default()
}

#[cfg(windows)]
fn read_child(pid: u32) -> Sample {
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
        else {
            return Sample::default();
        };
        let mut c = PROCESS_MEMORY_COUNTERS_EX::default();
        c.cb = std::mem::size_of_val(&c) as u32;
        let _ = GetProcessMemoryInfo(handle, &mut c as *mut _ as *mut _, c.cb);
        let (mut created, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let _ = GetProcessTimes(handle, &mut created, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        Sample {
            working_set_bytes: c.WorkingSetSize as u64,
            private_bytes: c.PrivateUsage as u64,
            cpu_seconds: (ticks(kernel) + ticks(user)) as f64 / 10_000_000.0,
            ..Default::default()
        }
    }
}
#[cfg(not(windows))]
fn read_child(_: u32) -> Sample {
    Sample::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_visual_headroom_without_claiming_the_original_target_passed() {
        let mut metrics = Metrics::default();
        metrics.samples.push(Sample {
            working_set_bytes: 201_000_000,
            private_bytes: 161_000_000,
            ..Default::default()
        });
        let report = metrics.report(serde_json::json!({}));
        assert_eq!(report["budget_bytes"], 256_000_000u64);
        assert_eq!(report["soft_target_bytes"], 200_000_000u64);
        assert_eq!(report["within_original_target"], false);
        assert_eq!(report["within_process_memory_budget"], true);
    }
}
