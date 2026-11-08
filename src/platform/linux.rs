use super::traits::WindowAdapter;
use crate::{
    app::Rimflux,
    core::config::Config,
    message::Message,
    ui::{notch, theme},
};
use iced::{Element, Size, Task, window};
use iced_layershell::{
    actions::ActionCallback,
    reexport::{Anchor, KeyboardInteractivity, Layer},
    settings::{LayerShellSettings, Settings},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};

pub struct LayerWindow;

impl WindowAdapter for LayerWindow {
    fn settings(&self, size: Size) -> window::Settings {
        window::Settings {
            size,
            ..Default::default()
        }
    }
    // The layer runtime handles size/input requests below, outside winit.
    fn synchronize(&self, _: window::Id, _: Size) -> Task<Message> {
        Task::none()
    }
}

struct Probe;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Probe {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// Query the actual Wayland registry; compositor names are not capability detection.
pub fn layer_shell_supported() -> Result<bool, Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;
    let (globals, _) = registry_queue_init::<Probe>(&connection)?;
    Ok(globals.contents().with_list(|globals| {
        globals
            .iter()
            .any(|global| global.interface == "zwlr_layer_shell_v1")
    }))
}

#[iced_layershell::to_layer_message]
#[derive(Debug, Clone)]
pub enum LayerMessage {
    App(Message),
}

struct LayerApp {
    app: Rimflux,
    last_size: Size,
}

fn input_region(size: Size) -> Task<LayerMessage> {
    // Region coordinates are logical surface units, independent of output scale.
    let spans: Vec<_> = (0..size.height.ceil() as i32)
        .filter_map(|y| {
            notch::span(size, y as f32 + 0.5)
                .map(|(left, right)| (y, left.ceil() as i32, right.floor() as i32))
        })
        .collect();
    Task::done(LayerMessage::SetInputRegion(ActionCallback::new(
        move |region| {
            for &(y, left, right) in &spans {
                if right > left {
                    region.add(left, y, right - left, 1);
                }
            }
        },
    )))
}

fn update(state: &mut LayerApp, message: LayerMessage) -> Task<LayerMessage> {
    let LayerMessage::App(message) = message else {
        return Task::none();
    };
    let task = state.app.update(message).map(LayerMessage::App);
    let size = state.app.state.size();
    let size = Size::new(size.width.ceil(), size.height.ceil());
    if size != state.last_size {
        state.last_size = size;
        Task::batch([
            task,
            Task::done(LayerMessage::SizeChange((
                size.width as u32,
                size.height as u32,
            ))),
            input_region(size),
        ])
    } else {
        task
    }
}

fn view(state: &LayerApp) -> Element<'_, LayerMessage> {
    state.app.view().map(LayerMessage::App)
}

pub fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let initial =
        crate::core::state::AppState::new(config.clone(), std::time::Instant::now()).size();
    tracing::info!(
        "Wayland layer-shell advertised; starting bounded top-anchored surface (no exclusive desktop area)"
    );
    iced_layershell::disable_clipboard();
    iced_layershell::application(move || {
        let (app, task) = Rimflux::boot(config.clone(), false, true);
        (LayerApp { app, last_size: initial }, Task::batch([task.map(LayerMessage::App), input_region(initial)]))
    }, "rimflux", update, view)
        .settings(Settings {
            antialiasing: true,
            layer_settings: LayerShellSettings {
                anchor: Anchor::Top, layer: Layer::Top,
                exclusive_zone: -1, size: Some((initial.width as u32, initial.height as u32)),
                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                ..Default::default()
            }, ..Default::default()
        })
        .theme(|_: &LayerApp| iced::Theme::Dark)
        .style(|_: &LayerApp, _: &iced::Theme| iced::theme::Style { background_color: iced::Color::TRANSPARENT, text_color: theme::TEXT })
        .subscription(|state: &LayerApp| state.app.subscription().map(LayerMessage::App))
        .run().map_err(|error| format!("Wayland layer-shell startup/runtime failed: {error}. Run rimflux --preview for a regular window."))?;
    Ok(())
}
