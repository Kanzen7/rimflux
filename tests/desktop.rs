use rimflux::{
    core::{
        config::Config,
        desktop::{
            AutoHide, Foreground, HideFor, ModeSelection, RuntimeVisibility, Shortcut, Visibility,
            VisibilityPolicy, VisibilitySettings, classify_window, covers_monitor,
        },
        state::{Action, AppState, NotchExpansion, NotchMode, Panel},
    },
    modules::music::MusicInfo,
};
use std::time::{Duration, Instant};

#[test]
fn manual_and_pinned_modes_keep_expansion_and_ignore_new_playback() {
    let now = Instant::now();
    let mut state = AppState::new(Config::default(), now);
    state.action(Action::ToggleExpansion, now);
    assert_eq!(state.mode, NotchMode::Clock);
    assert_eq!(state.expansion, NotchExpansion::Expanded);
    state.action(Action::SwitchMode(NotchMode::Monitoring), now);
    assert_eq!(state.expansion, NotchExpansion::Expanded);
    assert_eq!(state.selection, ModeSelection::Manual);
    state.receive_media(MusicInfo::demo(), now);
    assert_eq!(state.mode, NotchMode::Monitoring);
    assert!(state.music.playing);

    state.action(Action::TogglePin, now);
    state.action(Action::SwitchMode(NotchMode::Clock), now);
    assert_eq!(state.selection, ModeSelection::Pinned);
    assert_eq!(
        AppState::new(state.config.clone(), now).mode,
        NotchMode::Clock
    );
    assert_eq!(state.expansion, NotchExpansion::Expanded);
    state.receive_media(MusicInfo::default(), now);
    state.receive_media(MusicInfo::demo(), now);
    assert_eq!(state.mode, NotchMode::Clock);

    state.action(Action::AutoMode, now);
    assert_eq!(state.mode, NotchMode::Music);
    assert_eq!(state.expansion, NotchExpansion::Compact);
}

#[test]
fn auto_keeps_paused_music_and_returns_home_after_stop() {
    let now = Instant::now();
    let mut state = AppState::new(Config::default(), now);
    let mut playing = MusicInfo::demo();
    playing.demo = false;
    state.receive_media(playing.clone(), now);
    assert_eq!(state.mode, NotchMode::Music);
    assert_eq!(state.expansion, NotchExpansion::Compact);
    playing.playing = false;
    state.receive_media(playing.clone(), now);
    state.advance(now + Duration::from_secs(20));
    assert_eq!(state.mode, NotchMode::Music);
    playing.stopped = true;
    let stop = now + Duration::from_secs(20);
    state.receive_media(playing, stop);
    state.advance(stop + Duration::from_secs(8));
    assert_eq!(state.mode, NotchMode::Clock);
}

#[test]
fn smart_visibility_waits_for_confirmed_hide_and_restore() {
    let now = Instant::now();
    let config = VisibilitySettings::default();
    let mut visibility = Visibility::default();
    visibility.recovery_available = true;
    let tick = |visibility: &mut Visibility, millis| {
        visibility.advance(now + Duration::from_millis(millis), &config, false);
        visibility.runtime()
    };
    visibility.foreground = Foreground::MaximizedBrowser;
    assert_eq!(tick(&mut visibility, 0), RuntimeVisibility::Visible);
    visibility.foreground = Foreground::Window;
    assert_eq!(tick(&mut visibility, 100), RuntimeVisibility::Visible);
    visibility.foreground = Foreground::MaximizedBrowser;
    assert_eq!(tick(&mut visibility, 120), RuntimeVisibility::Visible);
    assert_eq!(tick(&mut visibility, 319), RuntimeVisibility::Visible);
    assert_eq!(
        tick(&mut visibility, 320),
        RuntimeVisibility::Automatic(AutoHide::MaximizedBrowser)
    );
    visibility.foreground = Foreground::Desktop;
    assert!(!tick(&mut visibility, 400).visible());
    assert!(!tick(&mut visibility, 749).visible());
    assert!(tick(&mut visibility, 750).visible());
    assert!(!visibility.needs_debounce_tick());
}

#[test]
fn manual_hide_expires_into_current_policy_and_can_be_restored() {
    let now = Instant::now();
    let config = VisibilitySettings {
        policy: VisibilityPolicy::AlwaysVisible,
        ..VisibilitySettings::default()
    };
    let mut visibility = Visibility::default();
    visibility.recovery_available = true;
    visibility.hide(HideFor::Minutes15, now);
    let end = now + Duration::from_secs(900);
    assert_eq!(
        visibility.runtime(),
        RuntimeVisibility::Manual { until: Some(end) }
    );
    visibility.foreground = Foreground::Fullscreen;
    visibility.advance(end, &config, false);
    assert_eq!(
        visibility.runtime(),
        RuntimeVisibility::Automatic(AutoHide::Fullscreen)
    );
    visibility.restore(end, &config);
    assert!(!visibility.runtime().visible());
    visibility.foreground = Foreground::Desktop;
    visibility.seed(&config);
    visibility.hide(HideFor::Hour, end);
    assert_eq!(
        visibility.runtime(),
        RuntimeVisibility::Manual {
            until: Some(end + Duration::from_secs(3600))
        }
    );
    visibility.hide(HideFor::UntilShown, end);
    visibility.advance(end + Duration::from_secs(7200), &config, false);
    assert_eq!(
        visibility.runtime(),
        RuntimeVisibility::Manual { until: None }
    );
    visibility.restore(end + Duration::from_secs(7200), &config);
    assert!(visibility.runtime().visible());
}

#[test]
fn peek_renews_during_interaction_and_fullscreen_cancels_it() {
    let now = Instant::now();
    let config = VisibilitySettings::default();
    let mut visibility = Visibility::default();
    visibility.recovery_available = true;
    visibility.foreground = Foreground::MaximizedBrowser;
    visibility.seed(&config);
    visibility.restore(now, &config);
    assert_eq!(visibility.runtime(), RuntimeVisibility::Peek);
    visibility.advance(now + Duration::from_secs(7), &config, true);
    visibility.advance(now + Duration::from_secs(8), &config, false);
    assert_eq!(visibility.runtime(), RuntimeVisibility::Peek);
    visibility.advance(now + Duration::from_secs(15), &config, false);
    assert_eq!(
        visibility.runtime(),
        RuntimeVisibility::Automatic(AutoHide::MaximizedBrowser)
    );
    visibility.restore(now + Duration::from_secs(16), &config);
    visibility.foreground = Foreground::Fullscreen;
    visibility.advance(now + Duration::from_secs(17), &config, false);
    assert!(!visibility.runtime().visible());
    visibility.recovery_available = false;
    assert!(visibility.runtime().visible());
}

#[test]
fn desktop_only_has_its_own_policy() {
    let config = VisibilitySettings {
        policy: VisibilityPolicy::DesktopOnly,
        ..VisibilitySettings::default()
    };
    let mut visibility = Visibility::default();
    visibility.recovery_available = true;
    visibility.foreground = Foreground::Window;
    visibility.seed(&config);
    assert_eq!(
        visibility.runtime(),
        RuntimeVisibility::Automatic(AutoHide::AwayFromDesktop)
    );
    visibility.foreground = Foreground::Desktop;
    visibility.seed(&config);
    assert!(visibility.runtime().visible());
}

#[test]
fn explicit_settings_remain_accessible_over_a_maximized_browser_without_peek() {
    let now = Instant::now();
    let mut config = Config::default();
    config.visibility.allow_temporary_peek = false;
    let mut state = AppState::new(config, now);
    state.visibility.recovery_available = true;
    state.visibility.foreground = Foreground::MaximizedBrowser;
    state.visibility.seed(&state.config.visibility);
    assert!(!state.visible());
    state.action(Action::OpenPanel(Panel::Settings), now);
    assert!(state.visible());
    state.action(Action::ClosePanel, now);
    assert!(!state.visible());
    state.visibility.foreground = Foreground::Fullscreen;
    state.visibility.seed(&state.config.visibility);
    state.action(Action::OpenPanel(Panel::Settings), now);
    assert!(!state.visible());
}

#[test]
fn fullscreen_uses_monitor_edges_and_browser_names_use_exact_matches() {
    let monitor = [-1920, -144, 0, 936];
    assert!(covers_monitor([-1922, -144, 1, 936], monitor, false, false));
    assert!(!covers_monitor([-1922, -144, 1, 936], monitor, true, true));
    assert!(!covers_monitor(
        [-1920, -144, 0, 896],
        monitor,
        false,
        false
    ));
    for name in [
        "chrome.exe",
        "MSEDGE.EXE",
        "firefox.exe",
        "brave.exe",
        "opera.exe",
        "opera_gx.exe",
        "vivaldi.exe",
    ] {
        assert_eq!(
            classify_window(name, true, false),
            Foreground::MaximizedBrowser
        );
        assert_eq!(classify_window(name, false, false), Foreground::Window);
    }
    assert_eq!(
        classify_window("electron.exe", true, false),
        Foreground::Maximized
    );
    assert_eq!(
        classify_window("chrome.exe", true, true),
        Foreground::Fullscreen
    );
}

#[test]
fn legacy_preferences_and_shortcut_validation_work() {
    let now = Instant::now();
    let legacy =
        Config::parse("[notch]\ndefault_mode='monitoring'\n[music]\nauto_switch=false").unwrap();
    let state = AppState::new(legacy, now);
    assert_eq!(state.mode, NotchMode::Monitoring);
    assert_eq!(state.selection, ModeSelection::Manual);
    assert!(Config::parse(include_str!("../config/default.toml")).is_ok());
    assert!(Shortcut::parse(" Ctrl + Alt + R ").is_some());
    assert!(Shortcut::parse("Alt+F11").is_some());
    for invalid in [
        "R",
        "Shift+R",
        "Ctrl+Ctrl+R",
        "Ctrl+F12",
        "Ctrl+Alt+",
        "Win+R",
        "Hyper+R",
    ] {
        assert!(Shortcut::parse(invalid).is_none());
    }
    assert!(Config::parse("[visibility]\npeek_seconds=0").is_err());
}

#[test]
fn settings_preserve_unrelated_values_and_malformed_files() {
    let temp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("rimflux-part4-{}", std::process::id()));
    let path = temp.join("settings.toml");
    std::fs::create_dir_all(&temp).unwrap();
    let original = "[music]\nidle_delay_secs=41\n[music.waveform]\ninteractive_seek=false\n";
    std::fs::write(&path, original).unwrap();
    let mut draft = Config::parse(original).unwrap();
    draft.navigation.startup_mode = Some(NotchMode::Monitoring);
    draft.visibility.policy = VisibilityPolicy::DesktopOnly;
    draft.save_desktop_to(&path).unwrap();
    draft.appearance.reduced_motion = true;
    draft.save_desktop_to(&path).unwrap();
    let updated = Config::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(updated.navigation.startup_mode, Some(NotchMode::Monitoring));
    assert_eq!(updated.visibility.policy, VisibilityPolicy::DesktopOnly);
    assert!(updated.appearance.reduced_motion);
    assert_eq!(updated.music.idle_delay_secs, 41);
    assert!(!updated.music.waveform.interactive_seek);
    std::fs::write(&path, "[broken").unwrap();
    assert!(draft.save_desktop_to(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[broken");
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(temp).unwrap();
}

#[test]
fn compact_context_menu_and_expanded_navigation_do_not_change_playback() {
    use iced::{
        Point, Rectangle, mouse,
        widget::canvas::{self, Program},
    };
    use rimflux::{
        message::Message,
        ui::layout::{Interaction, NotchView, hit_action},
    };
    let now = Instant::now();
    let mut config = Config::default();
    config.development.demo_mode = true;
    config.appearance.reduced_motion = true;
    let mut state = AppState::new(config, now);
    let (message, _, status) = NotchView(&state)
        .update(
            &mut Interaction::default(),
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
            Rectangle::new(Point::ORIGIN, state.size()),
            mouse::Cursor::Available(Point::new(58.0, 12.0)),
        )
        .unwrap()
        .into_inner();
    assert_eq!(status, iced::event::Status::Captured);
    let Some(Message::Action(action @ Action::OpenPanel(Panel::Menu))) = message else {
        panic!("right click must open the menu");
    };
    state.action(action, now);
    assert_eq!(state.expansion, NotchExpansion::Compact);
    state.action(Action::Escape, now);
    assert!(state.panel.is_none());
    state.action(Action::ToggleExpansion, now);
    let button = Point::new(state.size().width - 32.0, 21.0);
    assert!(matches!(
        hit_action(&state, button),
        Action::OpenPanel(Panel::Menu)
    ));
    state.action(Action::SwitchMode(NotchMode::Music), now);
    assert_eq!(state.expansion, NotchExpansion::Expanded);
    assert!(state.music.playing);
}
