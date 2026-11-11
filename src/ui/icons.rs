//! Original supplied SVGs, cached once and rendered at logical desktop sizes.
use iced::{
    Point, Rectangle, Size,
    advanced::svg::Svg,
    widget::{canvas::Frame, svg::Handle},
};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum Icon {
    Play(bool),
    CompactPlayback(bool),
    Previous,
    Next,
    Shuffle,
    Repeat,
    RepeatOne,
    More,
    AudioEffects,
}

impl Icon {
    fn handle(self) -> &'static Handle {
        macro_rules! asset {
            ($file:literal) => {{
                static HANDLE: OnceLock<Handle> = OnceLock::new();
                HANDLE.get_or_init(|| {
                    Handle::from_memory(include_bytes!(concat!(
                        "../../assets/icons/",
                        $file,
                        ".svg"
                    )))
                })
            }};
        }
        match self {
            Self::Play(true) => asset!("03_pause_active"),
            Self::Play(false) => asset!("07_play_active"),
            Self::CompactPlayback(true) => asset!("compact_pause"),
            Self::CompactPlayback(false) => asset!("compact_play"),
            Self::Previous => asset!("02_previous"),
            Self::Next => asset!("04_next"),
            Self::Shuffle => asset!("01_shuffle"),
            Self::Repeat => asset!("05_repeat"),
            Self::RepeatOne => asset!("08_repeat_one"),
            Self::More => asset!("06_more_vertical"),
            Self::AudioEffects => asset!("09_audio_effects_disabled"),
        }
    }
}

pub fn draw(frame: &mut Frame, icon: Icon, center: Point, size: f32, opacity: f32) {
    frame.draw_svg(
        Rectangle::new(
            Point::new(center.x - size / 2.0, center.y - size / 2.0),
            Size::new(size, size),
        ),
        Svg::new(icon.handle().clone()).opacity(opacity),
    );
}
