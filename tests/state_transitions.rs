use rimflux::{
    core::{
        config::Config,
        state::{Action, AppState, NotchExpansion::*, NotchMode::*, dimensions},
    },
    modules::{
        monitoring::{HISTORY_LENGTH, MonitoringInfo, SystemSnapshot},
        music::{MediaCommand, Repeat},
    },
    ui::{animations::TRANSITION_DURATION, layout::hit_action, notch},
};
use std::time::{Duration, Instant};

#[test]
fn all_five_states_reach_their_logical_geometry() {
    let start = Instant::now();
    let mut state = AppState::new(demo_config(), start);
    for (index, (mode, expansion)) in [
        (Clock, Compact),
        (Music, Compact),
        (Music, Expanded),
        (Monitoring, Compact),
        (Monitoring, Expanded),
    ]
    .into_iter()
    .enumerate()
    {
        let now = start + Duration::from_secs(index as u64);
        state.action(Action::Select(mode, expansion), now);
        state.advance(now + TRANSITION_DURATION);
        assert_eq!((state.mode, state.expansion), (mode, expansion));
        assert_eq!(state.size(), dimensions(mode, expansion));
        assert!(!state.animation.active);
    }
}

#[test]
fn playback_uses_elapsed_monotonic_time_and_pause_is_stable() {
    let start = Instant::now();
    let mut state = AppState::new(demo_config(), start);
    state.advance(start + Duration::from_millis(2500));
    assert_eq!(state.music.position, 86.5);
    state.action(
        Action::Media(MediaCommand::PlayPause),
        start + Duration::from_millis(2500),
    );
    state.advance(start + Duration::from_secs(6));
    assert_eq!(state.music.position, 86.5);
    assert!(!state.music.playing);
}

#[test]
fn media_navigation_shuffle_repeat_and_seek_respond() {
    let mut music = rimflux::modules::music::MusicInfo::demo();
    music.command(MediaCommand::Next);
    assert_eq!(music.title, "Every Summertime");
    music.command(MediaCommand::Previous);
    assert_eq!(music.title, "Autumn");
    music.command(MediaCommand::Shuffle);
    music.command(MediaCommand::Next);
    assert_eq!(music.title, "Oceans & Engines");
    music.command(MediaCommand::Repeat);
    assert_eq!(music.repeat, Repeat::All);
    music.command(MediaCommand::Repeat);
    assert_eq!(music.repeat, Repeat::One);
    music.command(MediaCommand::Seek(1.0));
    music.advance(Duration::from_secs(2));
    assert_eq!(music.title, "Oceans & Engines");
    assert_eq!(music.position, 2.0);
    music.command(MediaCommand::Seek(f32::NAN));
    assert_eq!(music.position, 2.0);
    music.command(MediaCommand::Seek(-10.0));
    assert_eq!(music.position, 0.0);
}

#[test]
fn hidden_and_paused_waveforms_have_no_frame_subscription() {
    let start = Instant::now();
    let mut state = AppState::new(demo_config(), start);
    assert!(state.frame_interval().is_none());
    state.action(Action::Select(Music, Compact), start);
    state.advance(start + TRANSITION_DURATION);
    assert!(state.frame_interval().is_some()); // Visible, playing procedural waveform.
    state.action(
        Action::Media(MediaCommand::PlayPause),
        start + TRANSITION_DURATION,
    );
    assert!(state.frame_interval().is_none());
    let phase = state.waveform.animation.phase;
    state.advance(start + Duration::from_secs(3));
    assert_eq!(state.waveform.animation.phase, phase);
}

#[test]
fn reduced_motion_snaps_geometry_and_disables_wave_animation() {
    let mut config = demo_config();
    config.appearance.reduced_motion = true;
    let now = Instant::now();
    let mut state = AppState::new(config, now);
    state.action(Action::Select(Music, Expanded), now);
    assert_eq!(state.size(), dimensions(Music, Expanded));
    assert!(state.frame_interval().is_none());
}

#[test]
fn expansion_grows_width_before_height_and_can_be_retargeted() {
    let now = Instant::now();
    let mut state = AppState::new(demo_config(), now);
    state.action(Action::Select(Music, Expanded), now);
    state.advance(now + Duration::from_millis(50));
    assert!(state.size().width > 116.0);
    assert_eq!(state.size().height, 27.0);
    let intermediate = state.size();
    state.action(
        Action::Select(Monitoring, Expanded),
        now + Duration::from_millis(50),
    );
    assert_eq!(state.size(), intermediate);
    state.advance(now + Duration::from_secs(1));
    assert_eq!(state.size(), dimensions(Monitoring, Expanded));
}

#[test]
fn auto_collapse_waits_for_pointer_leave_and_respects_preference() {
    let now = Instant::now();
    let mut state = AppState::new(demo_config(), now);
    state.action(Action::Select(Music, Expanded), now);
    state.action(Action::Hover(true), now);
    state.advance(now + Duration::from_secs(12));
    assert_eq!(state.expansion, Expanded);
    state.action(Action::Hover(false), now + Duration::from_secs(12));
    state.advance(now + Duration::from_secs(19));
    assert_eq!(state.expansion, Expanded);
    state.advance(now + Duration::from_secs(20));
    assert_eq!(state.expansion, Compact);
    state.config.notch.auto_collapse = false;
    state.action(Action::ToggleExpansion, now + Duration::from_secs(21));
    state.advance(now + Duration::from_secs(50));
    assert_eq!(state.expansion, Expanded);
}

#[test]
fn click_targets_activate_controls_without_collapsing_music() {
    let now = Instant::now();
    let mut state = AppState::new(demo_config(), now);
    state.action(Action::Select(Music, Expanded), now);
    let now = now + TRANSITION_DURATION;
    state.advance(now); // Seeking is intentionally disabled during a notch transition.
    let action = hit_action(&state, iced::Point::new(281.0, 103.0));
    state.action(action, now);
    assert!(!state.music.playing);
    assert_eq!(state.expansion, Expanded);
    state.action(hit_action(&state, iced::Point::new(284.0, 58.5)), now);
    assert!((state.music.progress() - 0.5).abs() < 0.001);
    state.action(hit_action(&state, iced::Point::new(424.0, 24.0)), now);
    assert_eq!(state.panel, Some(rimflux::core::state::Panel::Menu));
}

#[test]
fn notch_input_silhouette_excludes_transparent_corners() {
    for (mode, expansion) in [(Clock, Compact), (Music, Expanded), (Monitoring, Expanded)] {
        let size = dimensions(mode, expansion);
        assert!(notch::contains(
            size,
            iced::Point::new(size.width * 0.5, size.height * 0.5)
        ));
        assert!(!notch::contains(
            size,
            iced::Point::new(0.0, size.height - 1.0)
        ));
        assert!(!notch::contains(
            size,
            iced::Point::new(size.width - 1.0, size.height - 1.0)
        ));
        for point in notch::outline(size) {
            assert!(
                point.x >= 0.0
                    && point.x <= size.width + 0.001
                    && point.y >= 0.0
                    && point.y <= size.height + 0.001
            );
        }
    }
}

#[test]
fn monitoring_history_is_bounded_and_zero_memory_is_safe() {
    let mut monitoring = MonitoringInfo::default();
    for _ in 0..100 {
        monitoring.record(SystemSnapshot {
            cpu_percent: 25.0,
            ready: true,
            ..Default::default()
        });
    }
    assert_eq!(monitoring.cpu_history.len(), HISTORY_LENGTH);
    assert_eq!(monitoring.snapshot.ram_percent(), 0.0);
}

#[test]
fn partial_config_defaults_and_invalid_values_are_rejected() {
    let partial = Config::parse("[appearance]\nreduced_motion = true").unwrap();
    assert!(partial.appearance.reduced_motion);
    assert!(!partial.development.demo_mode);
    assert_eq!(partial.monitoring.refresh_interval_ms, 1000);
    for invalid in [
        "[",
        "[notch]\ndefault_mode = 'unknown'",
        "[monitoring]\nrefresh_interval_ms = 0",
        "[appearance]\ntheme = 'light'",
        "[music]\nwaveform_style = 'static'",
        "unexpected = true",
    ] {
        assert!(Config::parse(invalid).is_err(), "{invalid}");
    }
    assert!(Config::parse(include_str!("../config/default.toml")).is_ok());
}

#[test]
fn disabling_demo_never_exposes_fabricated_media_as_live() {
    let mut config = demo_config();
    config.development.demo_mode = false;
    let mut state = AppState::new(config, Instant::now());
    assert!(!state.music.available);
    state.action(Action::Media(MediaCommand::PlayPause), Instant::now());
    assert!(!state.music.playing);
}

fn demo_config() -> Config {
    let mut config = Config::default();
    config.development.demo_mode = true;
    config
}
