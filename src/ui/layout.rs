use super::{notch, theme::*, waveform};
use crate::{
    core::state::{Action, AppState, NotchExpansion, NotchMode},
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

pub struct NotchView<'a>(pub &'a AppState);

pub fn view(state: &AppState) -> Canvas<NotchView<'_>, Message> {
    Canvas::new(NotchView(state))
        .width(state.size().width)
        .height(state.size().height)
}

impl canvas::Program<Message> for NotchView<'_> {
    type State = bool;

    fn update(
        &self,
        hovered: &mut bool,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        let point = cursor.position_in(bounds);
        let inside = point.is_some_and(|p| notch::contains(self.0.size(), p));
        if let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && inside
            && !self.0.animation.active
        {
            return Some(iced::widget::Action::publish(Message::Action(hit_action(
                self.0, point?,
            ))));
        }
        if inside != *hovered {
            *hovered = inside;
            return Some(iced::widget::Action::publish(Message::Action(
                Action::Hover(inside),
            )));
        }
        None
    }

    fn draw(
        &self,
        _: &bool,
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
        let silhouette = notch::path(Size::new(size.width, size.height - 0.5));
        frame.fill(&silhouette, BASE);
        frame.stroke(
            &silhouette,
            Stroke::default().with_color(ELEVATED).with_width(0.8),
        );
        let opacity = state.animation.opacity(state.now);
        // Fade the target content through a bounded inset as the silhouette grows.
        // Text is never stretched, and remains outside the transparent shoulders.
        let background = frame.into_geometry();
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
            NotchMode::Music => painter.music(state),
            NotchMode::Monitoring => painter.monitoring(state),
        }
        vec![background, content.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &bool,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
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
    if state.mode == NotchMode::Music && state.music.available {
        if state.expansion == NotchExpansion::Compact {
            if Rectangle::new(Point::new(294.0, 7.0), Size::new(30.0, 28.0)).contains(point) {
                return Action::Media(MediaCommand::PlayPause);
            }
        } else {
            if (20.0..=430.0).contains(&point.x) && (74.0..=86.0).contains(&point.y) {
                return Action::Media(MediaCommand::Seek((point.x - 24.0) / 408.0));
            }
            for (center, command) in [
                (304.0, MediaCommand::Previous),
                (340.0, MediaCommand::PlayPause),
                (376.0, MediaCommand::Next),
                (266.0, MediaCommand::Shuffle),
                (412.0, MediaCommand::Repeat),
            ] {
                if Rectangle::new(Point::new(center - 14.0, 44.0), Size::new(28.0, 28.0))
                    .contains(point)
                {
                    return Action::Media(command);
                }
            }
            if Rectangle::new(Point::new(412.0, 12.0), Size::new(22.0, 24.0)).contains(point) {
                return Action::More;
            }
        }
    }
    Action::ToggleExpansion
}

struct Painter<'a> {
    frame: &'a mut Frame,
    opacity: f32,
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
            self.frame.fill(
                &Path::circle(Point::new(30.0, 12.0), 2.0),
                alpha(CYAN, self.opacity),
            );
            self.label(&state.clock.time, 42.0, 4.0, 13.0, TEXT, true);
        } else {
            self.label(&state.clock.time, 28.0, 10.0, 26.0, TEXT, true);
            self.label(&state.clock.date, 114.0, 18.0, 11.0, SECONDARY, false);
            self.label(
                "1  CLOCK     2 / 3  MUSIC     4 / 5  SYSTEM",
                28.0,
                51.0,
                9.0,
                SECONDARY,
                false,
            );
            self.label("RIMFLUX", 28.0, 67.0, 7.0, alpha(CYAN, 0.7), true);
        }
    }

    fn artwork(&mut self, x: f32, y: f32, side: f32) {
        self.frame.fill(
            &Path::rounded_rectangle(Point::new(x, y), Size::new(side, side), 5.0.into()),
            alpha(SURFACE, self.opacity),
        );
        // Original procedural album placeholder: dusk horizon, moon, and hills.
        self.frame.with_save(|frame| {
            frame.translate(Vector::new(x + 3.0, y + 3.0));
            let side = side - 6.0;
            frame.fill(
                &Path::rounded_rectangle(Point::ORIGIN, Size::new(side, side), 3.0.into()),
                alpha(Color::from_rgb8(53, 43, 72), self.opacity),
            );
            frame.fill(
                &Path::circle(Point::new(side * 0.64, side * 0.25), side * 0.14),
                alpha(Color::from_rgb8(229, 196, 160), self.opacity),
            );
            let hills = Path::new(|p| {
                p.move_to(Point::new(0.0, side * 0.68));
                p.quadratic_curve_to(
                    Point::new(side * 0.4, side * 0.22),
                    Point::new(side, side * 0.7),
                );
                p.line_to(Point::new(side - 2.0, side - 1.0));
                p.line_to(Point::new(2.0, side - 1.0));
                p.close();
            });
            frame.fill(&hills, alpha(Color::from_rgb8(141, 91, 86), self.opacity));
        });
    }

    fn wave(&mut self, state: &AppState, area: Rectangle) {
        if state.config.music.enable_waveform {
            waveform::draw(
                self.frame,
                area,
                state.phase,
                if state.music.playing { 1.0 } else { 0.22 },
                self.opacity,
                &waveform::LAYERS,
            );
        }
    }

    fn music(&mut self, state: &AppState) {
        let expanded = state.expansion == NotchExpansion::Expanded;
        if !state.music.available {
            self.label("MEDIA NOT CONNECTED", 26.0, 12.0, 11.0, SECONDARY, true);
            if expanded {
                self.label(
                    "Enable development.demo_mode for the media preview",
                    26.0,
                    52.0,
                    11.0,
                    SECONDARY,
                    false,
                );
            }
            return;
        }
        if !expanded {
            self.artwork(22.0, 7.0, 27.0);
            self.label(
                short_title(state.music.title, 19),
                59.0,
                5.0,
                11.0,
                TEXT,
                true,
            );
            self.label(
                format!("{}  /  DEMO", state.music.artist),
                59.0,
                22.0,
                8.0,
                SECONDARY,
                false,
            );
            self.wave(
                state,
                Rectangle::new(Point::new(190.0, 5.0), Size::new(94.0, 30.0)),
            );
            self.icon(Icon::Play(state.music.playing), 309.0, 21.0, TEXT);
        } else {
            self.artwork(24.0, 14.0, 53.0);
            self.label(
                short_title(state.music.title, 30),
                89.0,
                13.0,
                14.0,
                TEXT,
                true,
            );
            self.label(
                format!("{}  /  DEMO MEDIA", state.music.artist),
                89.0,
                34.0,
                9.0,
                SECONDARY,
                false,
            );
            self.wave(
                state,
                Rectangle::new(Point::new(88.0, 45.0), Size::new(155.0, 26.0)),
            );
            self.icon(
                Icon::More,
                424.0,
                24.0,
                if state.more { CYAN } else { SECONDARY },
            );
            self.icon(
                Icon::Shuffle,
                266.0,
                58.0,
                if state.music.shuffle { CYAN } else { SECONDARY },
            );
            self.icon(Icon::Previous, 304.0, 58.0, TEXT);
            self.frame.fill(
                &Path::circle(Point::new(340.0, 58.0), 13.0),
                alpha(ELEVATED, self.opacity),
            );
            self.icon(Icon::Play(state.music.playing), 340.0, 58.0, TEXT);
            self.icon(Icon::Next, 376.0, 58.0, TEXT);
            self.icon(
                Icon::Repeat,
                412.0,
                58.0,
                if state.music.repeat != Repeat::Off {
                    CYAN
                } else {
                    SECONDARY
                },
            );
            if state.music.repeat == Repeat::One {
                self.label("1", 410.0, 53.0, 7.0, CYAN, true);
            }
            self.line(
                Point::new(24.0, 80.0),
                Point::new(432.0, 80.0),
                ELEVATED,
                2.0,
            );
            self.line(
                Point::new(24.0, 80.0),
                Point::new(24.0 + 408.0 * state.music.progress(), 80.0),
                SECONDARY,
                2.0,
            );
            self.label(
                timestamp(state.music.position),
                24.0,
                89.0,
                9.0,
                SECONDARY,
                false,
            );
            self.label(
                timestamp(state.music.duration),
                404.0,
                89.0,
                9.0,
                SECONDARY,
                false,
            );
            if state.more {
                self.label(
                    "PROCEDURAL AUDIO  /  MOCK PLAYBACK",
                    112.0,
                    90.0,
                    8.0,
                    CYAN,
                    false,
                );
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
        let point = |dx, dy| Point::new(x + dx, y + dy);
        match icon {
            Icon::Play(true) => {
                for dx in [-3.0, 2.0] {
                    self.line(point(dx, -5.0), point(dx, 5.0), color, 2.0);
                }
            }
            Icon::Play(false) | Icon::Next | Icon::Previous => {
                let direction = if matches!(icon, Icon::Previous) {
                    -1.0
                } else {
                    1.0
                };
                let path = Path::new(|p| {
                    p.move_to(point(-3.0 * direction, -5.0));
                    p.line_to(point(4.0 * direction, 0.0));
                    p.line_to(point(-3.0 * direction, 5.0));
                    p.close();
                });
                self.frame.fill(&path, alpha(color, self.opacity));
                if !matches!(icon, Icon::Play(_)) {
                    self.line(
                        point(6.0 * direction, -5.0),
                        point(6.0 * direction, 5.0),
                        color,
                        1.5,
                    );
                }
            }
            Icon::More => {
                for dy in [-4.0, 0.0, 4.0] {
                    self.frame.fill(
                        &Path::circle(point(0.0, dy), 1.0),
                        alpha(color, self.opacity),
                    );
                }
            }
            Icon::Shuffle => {
                for direction in [-1.0, 1.0] {
                    self.line(
                        point(-6.0, -4.0 * direction),
                        point(6.0, 4.0 * direction),
                        color,
                        1.0,
                    );
                    self.line(
                        point(6.0, 4.0 * direction),
                        point(2.0, 4.0 * direction),
                        color,
                        1.0,
                    );
                    self.line(point(6.0, 4.0 * direction), point(6.0, 0.0), color, 1.0);
                }
            }
            Icon::Repeat => {
                for direction in [-1.0, 1.0] {
                    self.line(
                        point(-5.0 * direction, -4.0 * direction),
                        point(6.0 * direction, -4.0 * direction),
                        color,
                        1.0,
                    );
                    self.line(
                        point(6.0 * direction, -4.0 * direction),
                        point(3.0 * direction, -7.0 * direction),
                        color,
                        1.0,
                    );
                    self.line(
                        point(6.0 * direction, -4.0 * direction),
                        point(3.0 * direction, -direction),
                        color,
                        1.0,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Play(bool),
    Previous,
    Next,
    Shuffle,
    Repeat,
    More,
}

fn short_title(title: &str, limit: usize) -> String {
    if title.chars().count() <= limit {
        title.into()
    } else {
        format!("{}…", title.chars().take(limit - 1).collect::<String>())
    }
}
