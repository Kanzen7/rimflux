use iced::Color;
pub const BASE: Color = Color::BLACK;
pub const SURFACE: Color = Color::from_rgb8(21, 21, 27);
pub const ELEVATED: Color = Color::from_rgb8(30, 30, 38);
pub const TEXT: Color = Color::from_rgb8(244, 244, 247);
pub const SECONDARY: Color = Color::from_rgb8(164, 164, 178);
pub const CYAN: Color = Color::from_rgb8(0, 217, 245);
pub const VIOLET: Color = Color::from_rgb8(121, 85, 255);
pub const MAGENTA: Color = Color::from_rgb8(224, 43, 203);
pub fn alpha(color: Color, opacity: f32) -> Color {
    Color {
        a: color.a * opacity,
        ..color
    }
}
