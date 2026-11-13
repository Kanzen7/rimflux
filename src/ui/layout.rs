use super::{
    compact_playback::CompactPlaybackFeedback,
    icons::{self, Icon},
    music_layout::{CompactMusicLayout, WaveformSlot},
    notch,
    theme::*,
    waveform::{WaveformInteraction, WaveformPlayback, WaveformRenderer},
};
use crate::{
    core::state::{Action, AppState, NotchExpansion, NotchMode, Panel},
    message::Message,
    modules::{
        monitoring::rate,
        music::{MediaCommand, Repeat, timestamp},
    },
};
use iced::{
    Color, Font, Point, Rectangle, Renderer, Size, Theme, Vector, mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text},
};
use std::time::{Duration, Instant};

pub struct NotchView<'a>(pub &'a AppState);

pub fn view(state: &AppState) -> Canvas<NotchView<'_>, Message> {
    Canvas::new(NotchView(state))
        .width(state.size().width)
        .height(state.size().height)
}

impl canvas::Program<Message> for NotchView<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        let state = self.0;
        let slot = WaveformSlot::new(
            state.expansion,
            state.animation.target,
            &state.config.music.waveform,
        );
        let point = if matches!(event, canvas::Event::Mouse(mouse::Event::CursorLeft)) {
            None
        } else {
            cursor.position_in(bounds)
        };
        let inside = point.is_some_and(|p| notch::contains(state.size(), p));
        if inside
            && state.config.interaction.right_click_context_menu
            && matches!(
                event,
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            )
        {
            interaction.playback.pressed = false;
            interaction.waveform.dragging = false;
            return Some(
                iced::widget::Action::publish(Message::Action(Action::OpenPanel(Panel::Menu)))
                    .and_capture(),
            );
        }
        let menu_hover = inside
            && !state.animation.active
            && point.is_some_and(|p| menu_bounds(state).is_some_and(|bounds| bounds.contains(p)));
        let menu_changed = interaction.menu_hover != menu_hover;
        interaction.menu_hover = menu_hover;
        let now = match event {
            canvas::Event::Window(iced::window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };
        let compact = state.mode == NotchMode::Music
            && state.expansion == NotchExpansion::Compact
            && state.music.available;
        let playback_bounds = CompactMusicLayout::new(state.animation.target).playback_hitbox;
        let playback_over = compact
            && inside
            && !state.animation.active
            && point.is_some_and(|p| playback_bounds.contains(p));
        let playback_enabled = state.music.supports(MediaCommand::PlayPause);
        let playback_changed = interaction.playback.hover(
            playback_over && playback_enabled,
            now,
            state.config.appearance.reduced_motion,
        );
        if matches!(event, canvas::Event::Window(iced::window::Event::Unfocused)) {
            interaction.playback.pressed = false;
            interaction
                .playback
                .hover(false, now, state.config.appearance.reduced_motion);
            // Let an existing seek gesture follow its normal cancellation path.
            if !interaction.waveform.dragging {
                return Some(iced::widget::Action::request_redraw());
            }
        }
        if interaction.playback.pressed {
            if !compact
                || !playback_enabled
                || interaction.playback.generation != state.music.generation
                || state.animation.active
            {
                interaction.playback.pressed = false;
                return Some(iced::widget::Action::request_redraw().and_capture());
            }
            if matches!(
                event,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            ) {
                interaction.playback.pressed = false;
                return Some(
                    iced::widget::Action::publish(Message::Action(if playback_over {
                        Action::Media(MediaCommand::PlayPause)
                    } else {
                        Action::Hover(inside)
                    }))
                    .and_capture(),
                );
            }
            if matches!(
                event,
                canvas::Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
            ) {
                return Some(iced::widget::Action::request_redraw().and_capture());
            }
        }
        if playback_over
            && !interaction.waveform.dragging
            && matches!(
                event,
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
        {
            interaction.playback.pressed = playback_enabled;
            interaction.playback.generation = state.music.generation;
            return Some(iced::widget::Action::request_redraw().and_capture());
        }
        if compact
            && matches!(
                event,
                canvas::Event::Window(iced::window::Event::RedrawRequested(_))
            )
            && interaction.playback.animating(now)
        {
            return Some(iced::widget::Action::request_redraw_at(
                now + Duration::from_millis(16),
            ));
        }
        let seek = state.mode == NotchMode::Music && state.can_seek_waveform();
        let publish = |action| Some(iced::widget::Action::publish(Message::Action(action)));
        if interaction.waveform.dragging
            && (interaction.waveform.track_seed != state.waveform.profile.seed || !seek)
        {
            interaction.waveform.dragging = false;
            return publish(Action::SeekCancel);
        }
        if interaction.waveform.dragging {
            if matches!(
                event,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            ) {
                interaction.waveform.dragging = false;
                return publish(Action::SeekCommit);
            }
            if matches!(event, canvas::Event::Window(iced::window::Event::Unfocused)) {
                interaction.waveform.dragging = false;
                return publish(Action::SeekCancel);
            }
            if matches!(
                event,
                canvas::Event::Mouse(mouse::Event::CursorMoved { .. })
            ) && let Some(p) = cursor
                .position()
                .map(|p| p - Vector::new(bounds.x, bounds.y))
            {
                return publish(Action::SeekPreview(slot.ratio(p)));
            }
        }
        if matches!(
            event,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && inside
            && !state.animation.active
        {
            if let Some(p) = point
                && state.mode == NotchMode::Music
                && state.music.available
                && state.waveform_enabled()
                && slot.hitbox.contains(p)
            {
                if seek {
                    interaction.waveform.dragging = true;
                    interaction.waveform.track_seed = state.waveform.profile.seed;
                    return publish(Action::SeekPreview(slot.ratio(p)));
                }
                return publish(Action::Noop);
            }
            return publish(hit_action(state, point?));
        }
        if matches!(
            event,
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
        ) {
            let wave_hover = seek && point.is_some_and(|p| slot.hitbox.contains(p));
            if wave_hover || interaction.waveform.hovered {
                interaction.waveform.hovered = wave_hover;
                interaction.hovered = inside;
                return publish(Action::WaveHover(
                    if wave_hover {
                        point.map(|p| slot.ratio(p))
                    } else {
                        None
                    },
                    inside,
                ));
            }
        }
        if inside != interaction.hovered {
            interaction.hovered = inside;
            return publish(Action::Hover(inside));
        }
        if playback_changed || menu_changed {
            return Some(iced::widget::Action::request_redraw());
        }
        None
    }

    fn draw(
        &self,
        interaction: &Interaction,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let state = self.0;
        let size = Size::new(
            bounds.width.min(state.size().width),
            bounds.height.min(state.size().height),
        );
        let mut frame = Frame::new(renderer, size);
        let silhouette = notch::path(size);
        frame.fill(&silhouette, BASE);
        let opacity = if size.width >= state.animation.target.width - 0.5
            && size.height >= state.animation.target.height - 0.5
        {
            state.animation.opacity(state.now)
        } else {
            0.0
        };
        // Fade the target content through a bounded inset as the silhouette grows.
        // Text is never stretched, and remains outside the transparent shoulders.
        let background = frame.into_geometry();
        if opacity <= 0.0 {
            return vec![background];
        }
        let mut content = Frame::with_bounds(
            renderer,
            Rectangle::new(
                Point::new(12.0, 0.0),
                Size::new((size.width - 24.0).max(1.0), (size.height - 6.0).max(1.0)),
            ),
        );
        content.translate(Vector::new(
            (size.width - state.animation.target.width) * 0.5,
            0.0,
        ));
        let mut painter = Painter {
            frame: &mut content,
            opacity,
        };
        match state.mode {
            NotchMode::Clock => painter.clock(state),
            NotchMode::Music => painter.music(state, &interaction.playback),
            NotchMode::Monitoring => painter.monitoring(state),
        }
        if let Some(menu) = menu_bounds(state) {
            let center = menu.center();
            painter.icon(
                Icon::More,
                center.x,
                center.y,
                if interaction.menu_hover {
                    TEXT
                } else {
                    SECONDARY
                },
            );
            if interaction.menu_hover {
                let tooltip = Point::new(menu.x - 91.0, menu.y + menu.height + 1.0);
                painter.frame.fill(
                    &Path::rounded_rectangle(tooltip, Size::new(114.0, 20.0), 5.0.into()),
                    alpha(ELEVATED, opacity),
                );
                painter.label(
                    "Modes and settings",
                    tooltip.x + 7.0,
                    tooltip.y + 4.0,
                    9.0,
                    TEXT,
                    false,
                );
            }
        }
        let mut geometry = vec![background, content.into_geometry()];
        if state.mode == NotchMode::Music && state.music.available && state.waveform_enabled() {
            let slot = WaveformSlot::new(
                state.expansion,
                state.animation.target,
                &state.config.music.waveform,
            )
            .offset((size.width - state.animation.target.width) * 0.5);
            geometry.extend(WaveformRenderer::draw(
                renderer,
                slot,
                &state.waveform,
                &state.config.music.waveform,
                WaveformPlayback {
                    progress: state.waveform_progress(),
                    hover: state.wave_hover,
                    playing: state.music.playing,
                    reduced_motion: state.config.appearance.reduced_motion,
                    now: state.now,
                    opacity,
                },
            ));
        }
        geometry
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if interaction.waveform.dragging {
            return mouse::Interaction::Grabbing;
        }
        if self.0.mode == NotchMode::Music
            && self.0.expansion == NotchExpansion::Compact
            && self.0.music.available
            && cursor.position_in(bounds).is_some_and(|p| {
                CompactMusicLayout::new(self.0.animation.target)
                    .playback_hitbox
                    .contains(p)
            })
        {
            return if self.0.music.supports(MediaCommand::PlayPause) && !self.0.animation.active {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::default()
            };
        }
        let slot = WaveformSlot::new(
            self.0.expansion,
            self.0.animation.target,
            &self.0.config.music.waveform,
        );
        if self.0.mode == NotchMode::Music
            && self.0.music.available
            && self.0.waveform_enabled()
            && cursor
                .position_in(bounds)
                .is_some_and(|p| slot.hitbox.contains(p))
        {
            return if self.0.can_seek_waveform() {
                mouse::Interaction::Grab
            } else {
                mouse::Interaction::default()
            };
        }
        if cursor
            .position_in(bounds)
            .is_some_and(|p| notch::contains(self.0.size(), p))
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Shared coordinates keep drawing and hit targets in agreement.
pub fn hit_action(state: &AppState, point: Point) -> Action {
    if menu_bounds(state).is_some_and(|bounds| bounds.contains(point)) {
        return Action::OpenPanel(Panel::Menu);
    }
    if state.mode == NotchMode::Music && state.music.available {
        if state.expansion == NotchExpansion::Compact
            && CompactMusicLayout::new(state.animation.target)
                .playback_hitbox
                .contains(point)
        {
            return if state.music.supports(MediaCommand::PlayPause) {
                Action::Media(MediaCommand::PlayPause)
            } else {
                Action::Noop
            };
        }
        let slot = WaveformSlot::new(
            state.expansion,
            state.animation.target,
            &state.config.music.waveform,
        );
        if state.waveform_enabled() && slot.hitbox.contains(point) {
            return if state.can_seek_waveform() {
                Action::Media(MediaCommand::Seek(slot.ratio(point)))
            } else {
                Action::Noop
            };
        }
        if state.expansion == NotchExpansion::Expanded {
            if Rectangle::new(Point::new(141.0, 93.0), Size::new(20.0, 20.0)).contains(point) {
                return Action::Hover(state.hovered);
            }
            for (center, command) in [
                (234.0, MediaCommand::Previous),
                (281.0, MediaCommand::PlayPause),
                (330.0, MediaCommand::Next),
                (182.0, MediaCommand::Shuffle),
                (380.0, MediaCommand::Repeat),
            ] {
                if Rectangle::new(Point::new(center - 14.0, 90.0), Size::new(28.0, 28.0))
                    .contains(point)
                {
                    return Action::Media(command);
                }
            }
        }
    }
    if state.expansion == NotchExpansion::Compact && !state.config.interaction.compact_click_expands
    {
        Action::Noop
    } else {
        Action::ToggleExpansion
    }
}

fn menu_bounds(state: &AppState) -> Option<Rectangle> {
    (state.expansion == NotchExpansion::Expanded).then(|| {
        Rectangle::new(
            Point::new(state.animation.target.width - 44.0, 10.0),
            Size::new(24.0, 24.0),
        )
    })
}

struct Painter<'a> {
    frame: &'a mut Frame,
    opacity: f32,
}

#[derive(Default)]
pub struct Interaction {
    hovered: bool,
    waveform: WaveformInteraction,
    playback: CompactPlaybackFeedback,
    menu_hover: bool,
}

impl Painter<'_> {
    fn label(
        &mut self,
        content: impl Into<String>,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        bold: bool,
    ) {
        self.frame.fill_text(Text {
            content: content.into(),
            position: Point::new(x, y),
            size: size.into(),
            color: alpha(color, self.opacity),
            font: if bold {
                Font {
                    weight: iced::font::Weight::Semibold,
                    ..Font::DEFAULT
                }
            } else {
                Font::DEFAULT
            },
            ..Text::default()
        });
    }

    fn fitted_label(&mut self, title: &str, x: f32, y: f32, width: f32, size: f32, bold: bool) {
        let content = fit_text(title, width, size, bold);
        self.label(
            content,
            x,
            y,
            size,
            if bold { TEXT } else { SECONDARY },
            bold,
        );
    }

    fn line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        self.frame.stroke(
            &Path::line(from, to),
            Stroke::default()
                .with_color(alpha(color, self.opacity))
                .with_width(width),
        );
    }

    fn clock(&mut self, state: &AppState) {
        if state.expansion == NotchExpansion::Compact {
            self.frame.fill_text(Text {
                content: state.clock.time.clone(),
                position: Point::new(58.0, 13.5),
                size: 13.0.into(),
                color: alpha(TEXT, self.opacity),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Text::default()
            });
        } else {
            self.label(&state.clock.time, 28.0, 10.0, 26.0, TEXT, true);
            self.fitted_label(&state.clock.date, 28.0, 48.0, 264.0, 12.0, false);
        }
    }

    fn music(&mut self, state: &AppState, playback: &CompactPlaybackFeedback) {
        let music = &state.music;
        let expanded = state.expansion == NotchExpansion::Expanded;
        if !music.available {
            self.label("Apple Music", 25.0, 8.0, 12.0, TEXT, true);
            self.label("No media session", 25.0, 25.0, 8.0, SECONDARY, false);
            if expanded {
                self.label(
                    "Open Apple Music and start a song",
                    25.0,
                    60.0,
                    12.0,
                    SECONDARY,
                    false,
                );
            }
            return;
        }
        let control_color = |command| {
            if music.supports(command) {
                TEXT
            } else {
                alpha(SECONDARY, 0.28)
            }
        };
        let compact_row = CompactMusicLayout::new(state.animation.target);
        let (x, y, side) = if expanded {
            (25.0, 12.0, 104.0)
        } else {
            (
                compact_row.artwork.x,
                compact_row.artwork.y,
                compact_row.artwork.width,
            )
        };
        if let Some(artwork) = &music.artwork {
            let mut image = iced::advanced::image::Image::new(artwork.clone());
            image.border_radius = (if expanded { 15.0 } else { 5.0 }).into();
            image.opacity = self.opacity;
            self.frame.draw_image(
                Rectangle::new(Point::new(x, y), Size::new(side, side)),
                image,
            );
        } else {
            self.frame.fill(
                &Path::rounded_rectangle(
                    Point::new(x, y),
                    Size::new(side, side),
                    (if expanded { 15.0 } else { 5.0 }).into(),
                ),
                alpha(ELEVATED, self.opacity),
            );
            self.label(
                "♪",
                x + side * 0.38,
                y + side * 0.28,
                side * 0.3,
                SECONDARY,
                false,
            );
        }
        if !expanded {
            let info = compact_row.information;
            self.fitted_label(&music.title, info.x, info.y, info.width, 10.0, true);
            self.fitted_label(&music.artist, info.x, info.y + 15.0, info.width, 8.0, false);
            icons::draw(
                self.frame,
                Icon::CompactPlayback(music.playing),
                compact_row.playback_icon.center(),
                compact_row.playback_icon.width,
                self.opacity
                    * playback.opacity(Instant::now(), music.supports(MediaCommand::PlayPause)),
            );
            if music.demo {
                self.label("DEMO", 26.0, 34.0, 5.0, SECONDARY, false);
            }
        } else {
            self.fitted_label(&music.title, 150.0, 12.0, 254.0, 12.0, true);
            self.fitted_label(&music.artist, 150.0, 28.0, 268.0, 10.0, false);
            let progress = state.waveform_progress();
            self.label(
                timestamp(progress * music.duration),
                151.0,
                75.0,
                7.0,
                SECONDARY,
                false,
            );
            self.label(
                if music.duration > 0.0 {
                    timestamp(music.duration)
                } else {
                    "--:--".into()
                },
                397.0,
                75.0,
                7.0,
                SECONDARY,
                false,
            );
            self.icon(Icon::AudioEffects, 151.0, 103.0, TEXT);
            self.icon(
                Icon::Shuffle,
                182.0,
                103.0,
                if music.shuffle && music.capabilities.shuffle {
                    CYAN
                } else {
                    control_color(MediaCommand::Shuffle)
                },
            );
            self.icon(
                Icon::Previous,
                234.0,
                103.0,
                control_color(MediaCommand::Previous),
            );
            self.icon(
                Icon::Play(music.playing),
                281.0,
                103.0,
                control_color(MediaCommand::PlayPause),
            );
            self.icon(Icon::Next, 330.0, 103.0, control_color(MediaCommand::Next));
            self.icon(
                if music.repeat == Repeat::One {
                    Icon::RepeatOne
                } else {
                    Icon::Repeat
                },
                380.0,
                103.0,
                if music.repeat != Repeat::Off && music.capabilities.repeat {
                    CYAN
                } else {
                    control_color(MediaCommand::Repeat)
                },
            );
            if music.demo {
                self.label("DEMO", 25.0, 119.0, 6.0, SECONDARY, false);
            }
            if music.error.is_some() {
                self.label("!", 406.0, 18.0, 8.0, CYAN, true);
            }
        }
    }

    fn monitoring(&mut self, state: &AppState) {
        let data = &state.monitoring.snapshot;
        if !data.ready {
            self.label(
                if state.monitoring.error.is_some() {
                    "SYSTEM DATA UNAVAILABLE"
                } else {
                    "SAMPLING SYSTEM..."
                },
                27.0,
                12.0,
                10.0,
                SECONDARY,
                true,
            );
            return;
        }
        if state.expansion == NotchExpansion::Compact {
            self.label("CPU", 25.0, 7.0, 8.0, SECONDARY, false);
            self.label(
                format!("{:.0}%", data.cpu_percent),
                25.0,
                19.0,
                11.0,
                CYAN,
                true,
            );
            self.line(
                Point::new(88.0, 11.0),
                Point::new(88.0, 30.0),
                ELEVATED,
                1.0,
            );
            self.label("RAM", 101.0, 7.0, 8.0, SECONDARY, false);
            self.label(
                format!("{:.0}%", data.ram_percent()),
                101.0,
                19.0,
                11.0,
                VIOLET,
                true,
            );
            self.line(
                Point::new(167.0, 11.0),
                Point::new(167.0, 30.0),
                ELEVATED,
                1.0,
            );
            self.label("NET / DOWN", 180.0, 7.0, 8.0, SECONDARY, false);
            self.label(rate(data.download_bytes_sec), 180.0, 19.0, 10.0, TEXT, true);
        } else {
            self.label("SYSTEM", 26.0, 10.0, 9.0, TEXT, true);
            self.label(
                "LIVE  /  ALL INTERFACES",
                291.0,
                10.0,
                7.0,
                SECONDARY,
                false,
            );
            let columns = [
                (26.0, "CPU", format!("{:.0}%", data.cpu_percent), CYAN),
                (
                    162.0,
                    "MEMORY",
                    format!("{:.0}%", data.ram_percent()),
                    VIOLET,
                ),
                (298.0, "NETWORK", rate(data.download_bytes_sec), MAGENTA),
            ];
            for (x, label, value, color) in columns {
                self.label(label, x, 32.0, 8.0, SECONDARY, false);
                self.label(
                    value,
                    x,
                    46.0,
                    if x > 290.0 { 15.0 } else { 20.0 },
                    TEXT,
                    true,
                );
                let history = if x < 100.0 {
                    &state.monitoring.cpu_history
                } else if x < 290.0 {
                    &state.monitoring.ram_history
                } else {
                    &state.monitoring.network_history
                };
                let max = if x > 290.0 {
                    history.iter().copied().fold(1024.0, f32::max)
                } else {
                    100.0
                };
                let graph = Path::new(|p| {
                    let count = history.len().max(2) - 1;
                    for (i, value) in history.iter().enumerate() {
                        let point = Point::new(
                            x + i as f32 / count as f32 * 115.0,
                            92.0 - (value / max).clamp(0.0, 1.0) * 20.0,
                        );
                        if i == 0 {
                            p.move_to(point);
                        } else {
                            p.line_to(point);
                        }
                    }
                });
                self.line(
                    Point::new(x, 94.0),
                    Point::new(x + 115.0, 94.0),
                    ELEVATED,
                    1.0,
                );
                self.frame.stroke(
                    &graph,
                    Stroke::default()
                        .with_color(alpha(color, self.opacity))
                        .with_width(1.2),
                );
            }
            self.label("TOTAL UTILIZATION", 26.0, 102.0, 7.0, SECONDARY, false);
            self.label(
                format!(
                    "{:.1} / {:.1} GB",
                    data.used_memory as f64 / 1_073_741_824.0,
                    data.total_memory as f64 / 1_073_741_824.0
                ),
                162.0,
                102.0,
                7.0,
                SECONDARY,
                false,
            );
            self.label(
                format!("UP {}", rate(data.upload_bytes_sec)),
                298.0,
                102.0,
                7.0,
                SECONDARY,
                false,
            );
        }
    }

    fn icon(&mut self, icon: Icon, x: f32, y: f32, color: Color) {
        let size = match icon {
            Icon::Play(_) if x < 300.0 => 38.0,
            Icon::Play(_) => 22.0,
            Icon::More => 16.0,
            _ => 21.0,
        };
        icons::draw(
            self.frame,
            icon,
            Point::new(x, y),
            size,
            self.opacity * color.a,
        );
    }
}

// Measure actual shaped glyphs, with a small bounded UI-thread cache for stable metadata.
fn fit_text(title: &str, width: f32, size: f32, bold: bool) -> String {
    use iced::advanced::text::{Paragraph, Text as ParagraphText};
    type P = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph;
    type Entry = (String, f32, f32, bool, String);
    thread_local! { static CACHE: std::cell::RefCell<std::collections::VecDeque<Entry>> = const {std::cell::RefCell::new(std::collections::VecDeque::new())}; }
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.iter().find(|entry| {
            entry.0 == title && entry.1 == width && entry.2 == size && entry.3 == bold
        }) {
            return entry.4.clone();
        }
        let font = if bold {
            Font {
                weight: iced::font::Weight::Semibold,
                ..Font::DEFAULT
            }
        } else {
            Font::DEFAULT
        };
        let measure = |content: &str| {
            P::with_text(ParagraphText {
                content,
                bounds: Size::INFINITE,
                size: size.into(),
                line_height: iced::advanced::text::LineHeight::Relative(1.2),
                font,
                align_x: iced::advanced::text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::None,
            })
            .min_width()
        };
        let limited: String = title.chars().take(512).collect();
        let result = if limited.len() == title.len() && measure(&limited) <= width {
            limited
        } else {
            let chars: Vec<char> = limited.chars().collect();
            let (mut low, mut high) = (0, chars.len());
            while low < high {
                let mid = (low + high).div_ceil(2);
                let candidate = format!("{}…", chars[..mid].iter().collect::<String>());
                if measure(&candidate) <= width {
                    low = mid;
                } else {
                    high = mid - 1;
                }
            }
            format!("{}…", chars[..low].iter().collect::<String>())
        };
        if cache.len() >= 16 {
            cache.pop_front();
        }
        cache.push_back((title.into(), width, size, bold, result.clone()));
        result
    })
}
