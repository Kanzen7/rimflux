use std::collections::VecDeque;

pub const HISTORY_LENGTH: usize = 48;

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub cpu_percent: f32,
    pub used_memory: u64,
    pub total_memory: u64,
    pub download_bytes_sec: f64,
    pub upload_bytes_sec: f64,
    pub ready: bool,
}

impl SystemSnapshot {
    pub fn ram_percent(&self) -> f32 {
        if self.total_memory == 0 {
            0.0
        } else {
            self.used_memory as f32 / self.total_memory as f32 * 100.0
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MonitoringInfo {
    pub snapshot: SystemSnapshot,
    pub cpu_history: VecDeque<f32>,
    pub ram_history: VecDeque<f32>,
    pub network_history: VecDeque<f32>,
    pub error: Option<String>,
}

impl MonitoringInfo {
    pub fn record(&mut self, sample: SystemSnapshot) {
        for (history, value) in [
            (&mut self.cpu_history, sample.cpu_percent),
            (&mut self.ram_history, sample.ram_percent()),
            (
                &mut self.network_history,
                (sample.download_bytes_sec + sample.upload_bytes_sec) as f32,
            ),
        ] {
            if history.len() == HISTORY_LENGTH {
                history.pop_front();
            }
            history.push_back(value);
        }
        self.snapshot = sample;
        self.error = None;
    }
}

pub fn rate(bytes: f64) -> String {
    if bytes >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes / 1_048_576.0)
    } else {
        format!("{:.0} KB/s", bytes / 1024.0)
    }
}
