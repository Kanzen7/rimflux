use crate::modules::monitoring::SystemSnapshot;
use std::time::Instant;
use sysinfo::{Networks, System};

pub trait SystemProvider {
    fn sample(&mut self) -> SystemSnapshot;
}

/// Constructed without collection; all sampling takes place on a blocking worker.
#[derive(Default)]
pub struct NativeSystem {
    system: System,
    networks: Option<Networks>,
    last_sample: Option<Instant>,
}

impl SystemProvider for NativeSystem {
    fn sample(&mut self) -> SystemSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let now = Instant::now();
        let elapsed = self
            .last_sample
            .map(|last| now.duration_since(last).as_secs_f64());
        let networks = self
            .networks
            .get_or_insert_with(Networks::new_with_refreshed_list);
        networks.refresh(true);
        let (received, transmitted) =
            networks.iter().fold((0_u64, 0_u64), |(rx, tx), (_, data)| {
                (
                    rx.saturating_add(data.received()),
                    tx.saturating_add(data.transmitted()),
                )
            });
        self.last_sample = Some(now);
        let seconds = elapsed.unwrap_or(1.0).max(0.001);
        SystemSnapshot {
            cpu_percent: self.system.global_cpu_usage().clamp(0.0, 100.0),
            used_memory: self.system.used_memory(),
            total_memory: self.system.total_memory(),
            download_bytes_sec: if elapsed.is_some() {
                received as f64 / seconds
            } else {
                0.0
            },
            upload_bytes_sec: if elapsed.is_some() {
                transmitted as f64 / seconds
            } else {
                0.0
            },
            ready: elapsed.is_some(),
        }
    }
}
