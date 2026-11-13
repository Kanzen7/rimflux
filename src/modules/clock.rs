#[derive(Debug, Clone)]
pub struct ClockInfo {
    pub time: String,
    pub date: String,
}

pub trait ClockProvider {
    fn now(&self) -> ClockInfo;
}

pub struct LocalClock;

impl ClockProvider for LocalClock {
    fn now(&self) -> ClockInfo {
        let now = chrono::Local::now();
        ClockInfo {
            time: now.format("%H:%M").to_string(),
            date: now.format("%A, %d %B %Y").to_string(),
        }
    }
}
