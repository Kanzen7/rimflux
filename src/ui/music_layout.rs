use crate::core::{config::WaveformConfig, state::NotchExpansion};
use iced::{Point, Rectangle, Size};

/// Right-anchored compact row. Metadata truncates before the reserved playback
/// target; the waveform receives the remaining width. Values are logical px.
#[derive(Debug, Clone, Copy)]
pub struct CompactMusicLayout {
    pub artwork: Rectangle,
    pub information: Rectangle,
    pub timeline: Rectangle,
    pub playback_hitbox: Rectangle,
    pub playback_icon: Rectangle,
}
impl CompactMusicLayout {
    pub fn new(notch: Size) -> Self {
        let shoulder = 13.0_f32.min(notch.height * 0.4);
        let center_y = notch.height * 0.5;
        let artwork = Rectangle::new(Point::new(shoulder + 13.0, 10.0), Size::new(23.0, 23.0));
        let right = notch.width - shoulder - 12.0;
        let playback_hitbox = Rectangle::new(
            Point::new(right - 24.0, center_y - 12.0),
            Size::new(24.0, 24.0),
        );
        let center = playback_hitbox.center();
        let playback_icon = Rectangle::new(
            Point::new(center.x - 7.0, center.y - 7.0),
            Size::new(14.0, 14.0),
        );
        let text_x = artwork.x + artwork.width + 8.0;
        let timeline_right = playback_hitbox.x - 8.0;
        let available = (timeline_right - text_x - 8.0).max(0.0);
        let text_width = (available - 96.0).clamp(0.0, 79.0);
        let information = Rectangle::new(Point::new(text_x, 8.0), Size::new(text_width, 25.0));
        let timeline_left = information.x + information.width + 8.0;
        let timeline = Rectangle::new(
            Point::new(timeline_left, center_y - 12.0),
            Size::new((timeline_right - timeline_left).max(1.0), 24.0),
        );
        Self {
            artwork,
            information,
            timeline,
            playback_hitbox,
            playback_icon,
        }
    }
}

/// Shared logical-pixel allocation for painting and pointer-to-time mapping.
#[derive(Debug, Clone, Copy)]
pub struct WaveformSlot {
    pub visual: Rectangle,
    pub hitbox: Rectangle,
    pub expanded: bool,
}
impl WaveformSlot {
    pub fn new(expansion: NotchExpansion, notch: Size, config: &WaveformConfig) -> Self {
        let expanded = expansion == NotchExpansion::Expanded;
        let (left, width, center, limit, hit_height) = if expanded {
            (150.0, (notch.width - 188.0).max(1.0), 58.5, 25.0, 31.0)
        } else {
            let row = CompactMusicLayout::new(notch);
            (
                row.timeline.x,
                row.timeline.width,
                row.timeline.center_y(),
                14.0,
                row.timeline.height,
            )
        };
        let requested = if expanded {
            config.expanded_max_height
        } else {
            config.compact_max_height
        };
        let height = if requested.is_finite() {
            requested.clamp(1.0, limit)
        } else {
            limit
        };
        Self {
            visual: Rectangle::new(
                Point::new(left, center - height / 2.0),
                Size::new(width, height),
            ),
            hitbox: Rectangle::new(
                Point::new(left, center - hit_height / 2.0),
                Size::new(width, hit_height),
            ),
            expanded,
        }
    }
    pub fn ratio(&self, point: Point) -> f32 {
        ((point.x - self.visual.x) / self.visual.width).clamp(0.0, 1.0)
    }
    pub fn offset(mut self, x: f32) -> Self {
        self.visual.x += x;
        self.hitbox.x += x;
        self
    }
}
