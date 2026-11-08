use super::theme::{CYAN, MAGENTA, VIOLET, alpha};
use iced::{
    Color, Point, Rectangle,
    widget::canvas::{Frame, Path, Stroke},
};

#[derive(Debug, Clone, Copy)]
pub struct WaveLayer {
    pub amplitude: f32,
    pub frequency: f32,
    pub phase: f32,
    pub speed: f32,
    pub thickness: f32,
    pub opacity: f32,
    pub color: Color,
    pub visible: bool,
    pub ribbon: bool,
}

pub const LAYERS: [WaveLayer; 4] = [
    WaveLayer {
        amplitude: 0.73,
        frequency: 2.4,
        phase: 1.2,
        speed: -0.8,
        thickness: 0.8,
        opacity: 0.9,
        color: CYAN,
        visible: true,
        ribbon: false,
    },
    WaveLayer {
        amplitude: 0.64,
        frequency: 1.9,
        phase: 2.8,
        speed: 1.1,
        thickness: 0.9,
        opacity: 0.8,
        color: VIOLET,
        visible: true,
        ribbon: false,
    },
    WaveLayer {
        amplitude: 0.56,
        frequency: 2.0,
        phase: 0.0,
        speed: 1.0,
        thickness: 4.0,
        opacity: 0.9,
        color: MAGENTA,
        visible: true,
        ribbon: true,
    },
    WaveLayer {
        amplitude: 0.48,
        frequency: 2.7,
        phase: 2.0,
        speed: -0.7,
        thickness: 2.8,
        opacity: 0.38,
        color: CYAN,
        visible: true,
        ribbon: true,
    },
];

/// `amplitude` is the future audio-analysis input; it is procedural in Part 1.
pub fn draw(
    frame: &mut Frame,
    area: Rectangle,
    phase: f32,
    amplitude: f32,
    opacity: f32,
    layers: &[WaveLayer],
) {
    let count = (area.width as usize).clamp(24, 180);
    for layer in layers.iter().filter(|layer| layer.visible) {
        let sample = |index: usize, side: f32| {
            sample_point(
                *layer,
                area,
                index as f32 / count as f32,
                phase,
                amplitude,
                side,
            )
        };
        let path = Path::new(|path| {
            path.move_to(sample(0, if layer.ribbon { -1.0 } else { 0.0 }));
            for i in 1..=count {
                path.line_to(sample(i, if layer.ribbon { -1.0 } else { 0.0 }));
            }
            if layer.ribbon {
                for i in (0..=count).rev() {
                    path.line_to(sample(i, 1.0));
                }
                path.close();
            }
        });
        let color = alpha(layer.color, opacity * layer.opacity);
        if layer.ribbon {
            frame.fill(&path, color);
        } else {
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(color)
                    .with_width(layer.thickness),
            );
        }
    }
}

fn sample_point(
    layer: WaveLayer,
    area: Rectangle,
    x: f32,
    phase: f32,
    amplitude: f32,
    side: f32,
) -> Point {
    // sin(PI) can be slightly negative in f32; fractional powers require >= 0.
    let envelope = (std::f32::consts::PI * x).sin().max(0.0).powf(0.7);
    let wave = (std::f32::consts::TAU * layer.frequency * x + phase * layer.speed + layer.phase)
        .sin()
        * 0.72
        + (std::f32::consts::TAU * 3.2 * x - phase * 0.8 + layer.phase).sin() * 0.28;
    let y = area.height * 0.5
        + envelope * wave * area.height * 0.38 * layer.amplitude * amplitude.clamp(0.0, 1.0)
        + side * layer.thickness * envelope * 0.5;
    Point::new(
        area.x + x * area.width,
        area.y + y.clamp(1.0, area.height - 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_endpoints_and_ribbon_boundaries_stay_finite_and_inside_region() {
        for area in [
            Rectangle::new(Point::new(190.0, 5.0), iced::Size::new(94.0, 30.0)),
            Rectangle::new(Point::new(88.0, 45.0), iced::Size::new(155.0, 26.0)),
        ] {
            for layer in LAYERS {
                for phase in [0.0, 1.5, 628.0] {
                    for i in 0..=180 {
                        for side in [-1.0, 0.0, 1.0] {
                            let point =
                                sample_point(layer, area, i as f32 / 180.0, phase, 1.0, side);
                            assert!(point.x.is_finite() && point.y.is_finite());
                            assert!(point.x >= area.x && point.x <= area.x + area.width);
                            assert!(point.y >= area.y && point.y <= area.y + area.height);
                        }
                    }
                }
            }
        }
    }
}
