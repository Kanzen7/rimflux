//! Canvas-local feedback: no permanent timer, layout scaling, or media command.
use std::time::{Duration, Instant};
const FADE: Duration = Duration::from_millis(90);

#[derive(Debug, Default)]
pub struct CompactPlaybackFeedback {
    pub hovered: bool,
    pub pressed: bool,
    pub generation: u64,
    from: f32,
    target: f32,
    changed: Option<Instant>,
}
impl CompactPlaybackFeedback {
    pub fn hover(&mut self, hovered: bool, now: Instant, reduced: bool) -> bool {
        if self.hovered == hovered {
            return false;
        }
        self.from = self.level(now);
        self.hovered = hovered;
        self.target = if hovered { 1.0 } else { 0.0 };
        self.changed = if reduced {
            self.from = self.target;
            None
        } else {
            Some(now)
        };
        true
    }
    fn level(&self, now: Instant) -> f32 {
        self.changed.map_or(self.target, |at| {
            let t = (now.saturating_duration_since(at).as_secs_f32() / FADE.as_secs_f32())
                .clamp(0.0, 1.0);
            self.from + (self.target - self.from) * t * t * (3.0 - 2.0 * t)
        })
    }
    pub fn animating(&self, now: Instant) -> bool {
        self.changed
            .is_some_and(|at| now.saturating_duration_since(at) < FADE)
    }
    pub fn opacity(&self, now: Instant, enabled: bool) -> f32 {
        if !enabled {
            0.28
        } else {
            (0.78 + self.level(now) * 0.22)
                * if self.pressed && self.hovered {
                    0.72
                } else {
                    1.0
                }
        }
    }
}
