use iced::Size;
use std::time::{Duration, Instant};

pub const TRANSITION_DURATION: Duration = Duration::from_millis(320);

#[derive(Debug, Clone)]
pub struct Transition {
    from: Size,
    pub target: Size,
    started: Instant,
    pub active: bool,
}

impl Transition {
    pub fn new(size: Size, now: Instant) -> Self {
        Self {
            from: size,
            target: size,
            started: now,
            active: false,
        }
    }

    pub fn retarget(&mut self, target: Size, now: Instant, reduced_motion: bool) {
        self.from = self.size(now);
        self.target = target;
        self.started = now;
        self.active = !reduced_motion && self.from != target;
    }

    pub fn progress(&self, now: Instant) -> f32 {
        if !self.active {
            return 1.0;
        }
        (now.saturating_duration_since(self.started).as_secs_f32()
            / TRANSITION_DURATION.as_secs_f32())
        .clamp(0.0, 1.0)
    }

    pub fn size(&self, now: Instant) -> Size {
        let t = (self.progress(now) / 0.78).min(1.0);
        let horizontal = smooth(t);
        let vertical = smooth(((t - 0.22) / 0.78).clamp(0.0, 1.0));
        Size::new(
            self.from.width + (self.target.width - self.from.width) * horizontal,
            self.from.height + (self.target.height - self.from.height) * vertical,
        )
    }

    pub fn opacity(&self, now: Instant) -> f32 {
        smooth(((self.progress(now) - 0.78) / 0.22).clamp(0.0, 1.0))
    }
    pub fn tick(&mut self, now: Instant) {
        if self.progress(now) >= 1.0 {
            self.active = false;
        }
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
