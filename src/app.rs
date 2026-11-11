#[cfg(target_os = "windows")]
use crate::platform::windows_desktop::{self, DesktopHandle, Options};
use crate::{
    core::{
        config::Config,
        desktop::{HideFor, ModeSelection, RuntimeVisibility},
        state::{Action, AppState, NotchMode, Panel},
    },
    message::Message,
    modules::clock::{ClockProvider, LocalClock},
    platform::{self, traits::WindowAdapter},
    services::system::{NativeSystem, SystemProvider},
    ui,
};
use iced::{Element, Subscription, Task, window};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub struct Rimflux {
    pub state: AppState,
    pub preview: bool,
    pub layer_shell: bool,
    window_id: Option<window::Id>,
    adapter: Box<dyn WindowAdapter>,
    last_size: Option<iced::Size>,
    last_visible: Option<bool>,
    settings: Option<Config>,
    notice: Option<String>,
    desktop_notice: Option<String>,
    desktop_ready: bool,
    quitting: bool,
    system: Arc<Mutex<NativeSystem>>,
    sampling: bool,
    #[cfg(target_os = "windows")]
    media_tx: Option<std::sync::mpsc::SyncSender<crate::platform::windows_media::WorkerRequest>>,
    #[cfg(target_os = "windows")]
    desktop_initial: Options,
    #[cfg(target_os = "windows")]
    desktop: Option<DesktopHandle>,
}

impl Rimflux {
    pub fn boot(config: Config, preview: bool, layer_shell: bool) -> (Self, Task<Message>) {
        let state = AppState::new(config, Instant::now());
        (
            Self {
                #[cfg(target_os = "windows")]
                desktop_initial: Options::from(&state),
                state,
                preview,
                layer_shell,
                window_id: None,
                adapter: platform::adapter(preview),
                last_size: None,
                last_visible: None,
                settings: None,
                notice: None,
                desktop_notice: None,
                desktop_ready: !cfg!(target_os = "windows") || preview,
                quitting: false,
                system: Arc::new(Mutex::new(NativeSystem::default())),
                sampling: false,
                #[cfg(target_os = "windows")]
                media_tx: None,
                #[cfg(target_os = "windows")]
                desktop: None,
            },
            if layer_shell {
                Task::none()
            } else {
                window::oldest().then(|id| {
                    id.map_or_else(Task::none, |id| Task::done(Message::WindowReady(id)))
                })
            },
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let mut force_geometry = false;
        let now = Instant::now();
        match message {
            Message::AudioSnapshot(snapshot) => {
                self.state.receive_audio(snapshot, Instant::now());
            }
            Message::Action(action) => {
                let recovery = self.state.visibility.recovery_available;
                if !recovery && matches!(action, Action::Hide(_) | Action::ToggleVisibility) {
                    self.notice =
                        Some("Enable a tray icon or global shortcut before hiding.".into());
                    return Task::none();
                }
                if matches!(action, Action::OpenPanel(_)) {
                    self.state.hovered = false;
                }
                let command = match action {
                    crate::core::state::Action::Media(command) => Some(command),
                    crate::core::state::Action::SeekCommit => self
                        .state
                        .seek_preview
                        .map(crate::modules::music::MediaCommand::Seek),
                    _ => None,
                };
                if let Some(command) = command
                    && !self.state.music.demo
                    && self.state.music.supports(command)
                {
                    #[cfg(target_os = "windows")]
                    if let Some(tx) = &self.media_tx {
                        let request = crate::services::media::MediaRequest {
                            generation: self.state.music.generation,
                            command,
                        };
                        match tx.try_send(crate::platform::windows_media::WorkerRequest::Command(
                            request,
                        )) {
                            Ok(()) => {
                                self.state.music.busy = true;
                                self.state.music.error = None;
                            }
                            Err(_) => {
                                self.state.music.error =
                                    Some("Media service busy or disconnected; retry".into())
                            }
                        }
                    }
                }
                if let Some(command) = command
                    && self.state.music.demo
                    && matches!(action, crate::core::state::Action::SeekCommit)
                {
                    self.state
                        .action(crate::core::state::Action::Media(command), Instant::now());
                }
                self.state.action(action, now);
                if matches!(
                    action,
                    Action::SwitchMode(_)
                        | Action::AutoMode
                        | Action::TogglePin
                        | Action::SetVisibility(_)
                        | Action::ToggleFullscreenProtection
                ) && let Err(error) = Self::persist(&self.state.config)
                {
                    self.notice = Some(error);
                }
            }
            Message::SettingsEdit(edit) => {
                if let Some(config) = &mut self.settings {
                    edit.apply(config);
                    self.notice = None;
                    self.state.action(Action::Noop, now);
                }
            }
            Message::PanelHover(hovered) => {
                if self.state.panel.is_some() && self.state.hovered != hovered {
                    self.state.action(Action::Hover(hovered), now);
                }
            }
            Message::SaveSettings => {
                if let Some(mut config) = self.settings.clone() {
                    match self.save_settings(&mut config) {
                        Ok(()) => {
                            self.state.apply_config(config, now);
                            self.notice = None;
                        }
                        Err(error) => self.notice = Some(error),
                    }
                }
            }
            #[cfg(target_os = "windows")]
            Message::DesktopConnected(handle) => self.desktop = Some(handle),
            #[cfg(target_os = "windows")]
            Message::DesktopStatus(status) => {
                tracing::info!(foreground = ?status.foreground, tray = status.tray, hotkey = status.hotkey, error = ?status.error, "Desktop status");
                self.state.visibility.foreground = status.foreground;
                self.state.visibility.recovery_available = status.tray || status.hotkey;
                self.desktop_notice = status.error;
                if !self.desktop_ready {
                    self.state.visibility.seed(&self.state.config.visibility);
                    self.desktop_ready = true;
                }
                self.state.advance(now);
            }
            #[cfg(target_os = "windows")]
            Message::DesktopAction(action) => {
                if matches!(action, Action::SwitchMode(_) | Action::OpenPanel(_)) {
                    self.state.action(Action::Restore, now);
                }
                return self.update(Message::Action(action));
            }
            #[cfg(target_os = "windows")]
            Message::DesktopStopped => {
                self.desktop = None;
                self.state.visibility.recovery_available = false;
                self.desktop_ready = true;
                if self.quitting {
                    return self.adapter.shutdown();
                }
            }
            Message::MediaSnapshot(music) => self.state.receive_media(music, Instant::now()),
            Message::MediaCommandResult(result) => {
                self.state.music.busy = false;
                if result.is_err() {
                    self.state.cancel_seek();
                }
                self.state.music.error = result.err();
            }
            #[cfg(target_os = "windows")]
            Message::MediaConnected(tx) => self.media_tx = Some(tx),
            Message::Frame(now) | Message::Progress(now) => self.state.advance(now),
            Message::Second(now) => {
                self.state.advance(now);
                self.state.clock = LocalClock.now();
            }
            Message::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                let shortcut = if let Some(panel) = self.state.panel {
                    ui::desktop::shortcut(panel, &key, modifiers)
                } else {
                    platform::shortcut(
                        &key,
                        modifiers,
                        self.preview
                            || cfg!(debug_assertions)
                            || self.state.config.development.demo_mode,
                    )
                };
                if let Some(message) = shortcut {
                    return self.update(message);
                }
            }
            Message::Keyboard(_) => {}
            Message::Sample => {
                if !self.sampling && self.wants_metrics() {
                    self.sampling = true;
                    let provider = self.system.clone();
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                provider
                                    .lock()
                                    .map_err(|error| error.to_string())
                                    .map(|mut provider| provider.sample())
                            })
                            .await
                            .map_err(|error| error.to_string())
                            .and_then(|sample| sample)
                        },
                        Message::Metrics,
                    );
                }
            }
            Message::Metrics(result) => {
                self.sampling = false;
                match result {
                    Ok(sample) => self.state.monitoring.record(sample),
                    Err(error) => {
                        tracing::warn!(%error, "System sampling failed");
                        self.state.monitoring.error = Some(error);
                    }
                }
            }
            Message::WindowReady(id) => {
                self.window_id = Some(id);
                force_geometry = true;
            }
            Message::Display(display) => self.state.display = display,
            Message::Quit => {
                self.quitting = true;
                #[cfg(target_os = "windows")]
                if let Some(desktop) = &self.desktop {
                    desktop.stop();
                    return Task::none();
                }
                return self.adapter.shutdown();
            }
            Message::WindowEvent(window::Event::CloseRequested) => {
                if !self.native_desktop() {
                    return self.adapter.shutdown();
                }
                if self.state.visibility.recovery_available {
                    return self.update(Message::Action(Action::Hide(HideFor::UntilShown)));
                }
                self.notice = Some("Use Quit Rimflux to close the app.".into());
            }
            Message::WindowEvent(window::Event::Unfocused)
                if self.state.panel.is_none()
                    && self.state.visibility.runtime() == RuntimeVisibility::Peek =>
            {
                self.state.visibility.dismiss_peek();
            }
            Message::WindowEvent(window::Event::Rescaled(scale)) => {
                self.state.display.scale = scale;
                force_geometry = true;
            }
            Message::WindowEvent(_) => {}
        }
        if !self.state.wants_audio_capture() {
            self.state.waveform.audio.clear();
        }
        if self.state.panel == Some(Panel::Settings) {
            self.settings
                .get_or_insert_with(|| self.state.config.clone());
        } else {
            self.settings = None;
        }
        #[cfg(target_os = "windows")]
        if let Some(desktop) = &self.desktop {
            desktop.configure(&self.state);
        }
        if let Some(id) = self.window_id {
            let visible = self.state.visible() && self.desktop_ready && !self.quitting;
            let changed = self.last_visible != Some(visible);
            let was_visible = self.last_visible.replace(visible) == Some(true);
            if !visible {
                if was_visible {
                    self.state.on_hidden();
                    self.settings = None;
                    return self.adapter.visibility(id, false);
                }
                if force_geometry {
                    return self.adapter.synchronize(id, self.state.size(), false);
                }
                return Task::none();
            }
            let size = self.state.size();
            let size = iced::Size::new(size.width.ceil(), size.height.ceil());
            if changed || force_geometry || self.last_size != Some(size) {
                self.last_size = Some(size);
                return self
                    .adapter
                    .synchronize(id, size, changed && self.native_desktop());
            }
        }
        Task::none()
    }

    fn native_desktop(&self) -> bool {
        cfg!(target_os = "windows") && !self.preview && !self.layer_shell
    }

    fn wants_metrics(&self) -> bool {
        self.state.visible()
            && self.state.panel.is_none()
            && self.state.mode == NotchMode::Monitoring
    }

    fn persist(config: &Config) -> Result<(), String> {
        let path = Config::user_path().ok_or("No configuration path is available.")?;
        config.save_desktop_to(&path).map_err(|error| {
            tracing::warn!(%error, "Could not save desktop settings");
            format!("Could not save settings: {error}")
        })
    }

    fn save_settings(&self, config: &mut Config) -> Result<(), String> {
        config.validate().map_err(|error| error.to_string())?;
        let selection = config
            .navigation
            .mode_selection
            .unwrap_or(self.state.selection);
        if selection == ModeSelection::Pinned
            || (config.navigation.restore_last_mode && config.navigation.last_mode.is_none())
        {
            config.navigation.last_mode = Some(self.state.mode);
        }
        #[cfg(target_os = "windows")]
        let startup_changed = self.native_desktop()
            && config.general.start_with_windows != self.state.config.general.start_with_windows;
        #[cfg(target_os = "windows")]
        if startup_changed {
            windows_desktop::set_startup(config.general.start_with_windows)?;
        }
        let result = Self::persist(config);
        #[cfg(target_os = "windows")]
        if result.is_err()
            && startup_changed
            && let Err(error) =
                windows_desktop::set_startup(self.state.config.general.start_with_windows)
        {
            return Err(format!(
                "Settings were not saved. Startup rollback failed: {error}"
            ));
        }
        result
    }

    pub fn view(&self) -> Element<'_, Message> {
        if !self.state.visible() || !self.desktop_ready || self.quitting {
            return iced::widget::space().into();
        }
        if let Some(panel) = self.state.panel {
            let notice = self
                .notice
                .as_deref()
                .or(self.desktop_notice.as_deref())
                .or(self
                    .state
                    .music
                    .error
                    .as_deref()
                    .filter(|_| panel == Panel::Menu));
            return ui::desktop::view(
                panel,
                &self.state,
                self.settings.as_ref(),
                notice,
                self.native_desktop(),
            );
        }
        ui::layout::view(&self.state).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::time::every(Duration::from_secs(1)).map(Message::Second),
            iced::keyboard::listen().map(Message::Keyboard),
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Window(event) => Some(Message::WindowEvent(event)),
                _ => None,
            }),
        ];
        if self.state.panel.is_some() {
            subscriptions.push(iced::event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(
                    iced::mouse::Event::CursorEntered | iced::mouse::Event::CursorMoved { .. },
                ) => Some(Message::PanelHover(true)),
                iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                    Some(Message::PanelHover(false))
                }
                _ => None,
            }));
        }
        if let Some(interval) = self.state.frame_interval() {
            subscriptions.push(iced::time::every(interval).map(Message::Frame));
        }
        if self.state.visibility.needs_debounce_tick() {
            subscriptions.push(iced::time::every(Duration::from_millis(40)).map(Message::Frame));
        }
        if self.state.visible()
            && self.state.panel.is_none()
            && self.state.mode == NotchMode::Music
            && self.state.music.playing
            && !self.state.config.appearance.reduced_motion
            && !self.state.waveform_active()
        {
            subscriptions
                .push(iced::time::every(Duration::from_millis(100)).map(Message::Progress));
        }
        if self.wants_metrics() {
            subscriptions.push(
                iced::time::every(Duration::from_millis(
                    self.state.config.monitoring.refresh_interval_ms,
                ))
                .map(|_| Message::Sample),
            );
        }
        #[cfg(target_os = "windows")]
        if self.native_desktop() {
            subscriptions.push(windows_desktop::subscription(self.desktop_initial.clone()));
        }
        #[cfg(target_os = "windows")]
        if self.state.wants_audio_capture() {
            subscriptions.push(crate::platform::windows_audio::subscription(
                (&self.state.config.music.waveform).into(),
            ));
        }
        #[cfg(target_os = "windows")]
        if !self.state.config.development.demo_mode {
            subscriptions.push(crate::platform::windows_media::subscription(
                self.state.config.music.source_app_id.clone(),
            ));
        }
        Subscription::batch(subscriptions)
    }
}
