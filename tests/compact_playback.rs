//! Headless layout and real Canvas-event regressions; no playback/device access.
use iced::{
    Point, Rectangle, Size, mouse,
    widget::canvas::{self, Program},
};
use rimflux::{
    core::{
        config::Config,
        state::{Action, AppState, NotchExpansion, NotchMode, dimensions},
    },
    message::Message,
    modules::music::{Capabilities, MediaCommand},
    ui::{
        compact_playback::CompactPlaybackFeedback,
        layout::{Interaction, NotchView, hit_action},
        music_layout::{CompactMusicLayout, WaveformSlot},
        notch,
    },
};
use std::time::{Duration, Instant};

fn state() -> AppState {
    let mut config = Config::default();
    config.notch.default_mode = NotchMode::Music;
    config.development.demo_mode = true;
    AppState::new(config, Instant::now())
}
fn event(
    state: &AppState,
    interaction: &mut Interaction,
    event: canvas::Event,
    point: Option<Point>,
) -> Option<(
    Option<Message>,
    iced::window::RedrawRequest,
    iced::event::Status,
)> {
    NotchView(state)
        .update(
            interaction,
            &event,
            Rectangle::new(Point::ORIGIN, state.size()),
            point.map_or(mouse::Cursor::Unavailable, mouse::Cursor::Available),
        )
        .map(|action| action.into_inner())
}
fn down() -> canvas::Event {
    canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
}
fn up() -> canvas::Event {
    canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
}
fn media(message: &Option<Message>) -> bool {
    matches!(message, Some(Message::Action(Action::Media(_))))
}

#[test]
fn layout_reserves_control_and_right_padding_without_changing_notch_size() {
    let state = state();
    assert_eq!(state.size(), Size::new(345.0, 42.0));
    for width in [280.0, 300.0, 345.0, 456.0] {
        let size = Size::new(width, 42.0);
        let row = CompactMusicLayout::new(size);
        let slot = WaveformSlot::new(NotchExpansion::Compact, size, &state.config.music.waveform);
        assert_eq!(row.artwork.size(), Size::new(23.0, 23.0));
        assert_eq!(row.playback_hitbox.size(), Size::new(24.0, 24.0));
        assert_eq!(row.playback_icon.size(), Size::new(14.0, 14.0));
        assert!(slot.visual.width >= 96.0);
        assert!(row.information.x + row.information.width + 8.0 <= slot.visual.x);
        assert!(slot.hitbox.x + slot.hitbox.width + 8.0 <= row.playback_hitbox.x + 0.001);
        for bounds in [row.playback_hitbox, row.playback_icon] {
            for x in [bounds.x, bounds.x + bounds.width] {
                for y in [bounds.y, bounds.y + bounds.height] {
                    assert!(
                        notch::contains(size, Point::new(x, y)),
                        "control escaped rounded boundary"
                    );
                    // The content frame is inset 12px horizontally and 6px at bottom.
                    assert!(x >= 12.0 && x <= width - 12.0 && y <= size.height - 6.0);
                }
            }
        }
        assert!(
            (width - 13.0 - row.playback_hitbox.x - row.playback_hitbox.width - 12.0).abs() < 0.001
        );
    }
    let row = CompactMusicLayout::new(state.size());
    assert_eq!(row.information.width, 79.0);
    assert_eq!(row.timeline.width, 144.0);
}

#[test]
fn compact_click_is_captured_and_only_release_toggles_playback() {
    let mut state = state();
    state.music.title = "A very long track title with Unicode 音楽 🎵 ".repeat(40);
    state.music.artist = "An equally long artist name ".repeat(40);
    let mut interaction = Interaction::default();
    let button = CompactMusicLayout::new(state.size())
        .playback_hitbox
        .center();
    let (message, _, status) = event(&state, &mut interaction, down(), Some(button)).unwrap();
    assert!(message.is_none());
    assert_eq!(status, iced::event::Status::Captured);
    assert!(state.music.playing);
    let (message, _, status) = event(&state, &mut interaction, up(), Some(button)).unwrap();
    assert_eq!(status, iced::event::Status::Captured);
    let Some(Message::Action(Action::Media(MediaCommand::PlayPause))) = message else {
        panic!("release must publish one playback toggle");
    };
    state.action(Action::Media(MediaCommand::PlayPause), Instant::now());
    assert!(!state.music.playing);
    assert_eq!(state.expansion, NotchExpansion::Compact);
    let duplicate = event(&state, &mut interaction, up(), Some(button));
    assert!(duplicate.as_ref().is_none_or(|(m, _, _)| !media(m)));
    let (message, _, _) = event(
        &state,
        &mut interaction,
        down(),
        Some(Point::new(90.0, 21.0)),
    )
    .unwrap();
    assert!(matches!(
        message,
        Some(Message::Action(Action::ToggleExpansion))
    ));
}

#[test]
fn dragging_off_or_losing_focus_cancels_without_seeking_or_expanding() {
    let state = state();
    let button = CompactMusicLayout::new(state.size())
        .playback_hitbox
        .center();
    let mut interaction = Interaction::default();
    event(&state, &mut interaction, down(), Some(button));
    let point = Point::new(230.0, 21.0);
    let (message, _, status) = event(
        &state,
        &mut interaction,
        canvas::Event::Mouse(mouse::Event::CursorMoved { position: point }),
        Some(point),
    )
    .unwrap();
    assert!(message.is_none());
    assert_eq!(status, iced::event::Status::Captured);
    let (message, _, status) = event(&state, &mut interaction, up(), Some(point)).unwrap();
    assert!(!media(&message));
    assert_eq!(status, iced::event::Status::Captured);
    event(&state, &mut interaction, down(), Some(button));
    event(
        &state,
        &mut interaction,
        canvas::Event::Window(iced::window::Event::Unfocused),
        Some(button),
    );
    assert!(
        event(&state, &mut interaction, up(), Some(button))
            .as_ref()
            .is_none_or(|(m, _, _)| !media(m))
    );
}

#[test]
fn unsupported_or_busy_playback_is_disabled_and_does_not_expand() {
    let mut state = state();
    let button = CompactMusicLayout::new(state.size())
        .playback_hitbox
        .center();
    for busy in [false, true] {
        state.music.busy = busy;
        if !busy {
            state.music.capabilities = Capabilities::default();
        }
        assert!(matches!(hit_action(&state, button), Action::Noop));
        let mut interaction = Interaction::default();
        let (message, _, status) = event(&state, &mut interaction, down(), Some(button)).unwrap();
        assert!(!media(&message));
        assert_eq!(status, iced::event::Status::Captured);
        assert!(
            event(&state, &mut interaction, up(), Some(button))
                .as_ref()
                .is_none_or(|(m, _, _)| !media(m))
        );
        assert_eq!(
            NotchView(&state).mouse_interaction(
                &interaction,
                Rectangle::new(Point::ORIGIN, state.size()),
                mouse::Cursor::Available(button)
            ),
            mouse::Interaction::default()
        );
    }
}

#[test]
fn feedback_fades_brightness_without_scaling_and_stops_redrawing() {
    let start = Instant::now();
    let mut feedback = CompactPlaybackFeedback::default();
    assert!((feedback.opacity(start, true) - 0.78).abs() < 0.001);
    feedback.hover(true, start, false);
    let middle = feedback.opacity(start + Duration::from_millis(45), true);
    assert!(middle > 0.78 && middle < 1.0);
    assert!(feedback.animating(start + Duration::from_millis(45)));
    assert!(!feedback.animating(start + Duration::from_millis(100)));
    assert_eq!(
        feedback.opacity(start + Duration::from_millis(100), true),
        1.0
    );
    feedback.pressed = true;
    assert_eq!(
        feedback.opacity(start + Duration::from_millis(100), true),
        0.72
    );
    assert_eq!(feedback.opacity(start, false), 0.28);
    feedback.hover(false, start + Duration::from_millis(110), true);
    assert!(!feedback.animating(start + Duration::from_millis(110)));
    assert_eq!(
        dimensions(NotchMode::Music, NotchExpansion::Compact),
        Size::new(345.0, 42.0)
    );
}
