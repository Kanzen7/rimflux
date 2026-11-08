use crate::{
    core::{
        config::Config,
        state::{AppState, NotchMode},
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
    system: Arc<Mutex<NativeSystem>>,
    sampling: bool,
}

impl Rimflux {
    pub fn boot(config: Config, preview: bool, layer_shell: bool) -> (Self, Task<Message>) {
        (
            Self {
                state: AppState::new(config, Instant::now()),
                preview,
                layer_shell,
                window_id: None,
                adapter: platform::adapter(preview),
                last_size: None,
                system: Arc::new(Mutex::new(NativeSystem::default())),
                sampling: false,
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
        match message {
            Message::Action(action) => self.state.action(action, Instant::now()),
            Message::Frame(now) => self.state.advance(now),
            Message::Second(now) => {
                self.state.advance(now);
                self.state.clock = LocalClock.now();
            }
            Message::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(message) =
                    platform::shortcut(&key, modifiers, self.state.config.development.demo_mode)
                {
                    return self.update(message);
                }
            }
            Message::Keyboard(_) => {}
            Message::Sample => {
                if !self.sampling && self.state.mode == NotchMode::Monitoring {
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
            Message::WindowEvent(window::Event::CloseRequested) | Message::Quit => {
                return self.adapter.shutdown();
            }
            Message::WindowEvent(window::Event::Rescaled(scale)) => {
                self.state.display.scale = scale;
                force_geometry = true;
            }
            Message::WindowEvent(_) => {}
        }
        if let Some(id) = self.window_id {
            let size = self.state.size();
            let size = iced::Size::new(size.width.ceil(), size.height.ceil());
            if force_geometry || self.last_size != Some(size) {
                self.last_size = Some(size);
                let sync = self.adapter.synchronize(id, size);
                return if force_geometry {
                    Task::batch([sync, self.adapter.visibility(id, true)])
                } else {
                    sync
                };
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
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
        if let Some(interval) = self.state.frame_interval() {
            subscriptions.push(iced::time::every(interval).map(Message::Frame));
        }
        if self.state.mode == NotchMode::Monitoring {
            subscriptions.push(
                iced::time::every(Duration::from_millis(
                    self.state.config.monitoring.refresh_interval_ms,
                ))
                .map(|_| Message::Sample),
            );
        }
        Subscription::batch(subscriptions)
    }
}
