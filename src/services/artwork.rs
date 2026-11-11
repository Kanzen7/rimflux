/// Bake antialiased transparent corners once, off the UI thread. This also works
/// on Canvas backends that do not apply the image primitive's border radius.
pub fn round_corners(image: &mut image::RgbaImage) {
    let (width, height) = image.dimensions();
    let radius = width.min(height) as f32 * 15.0 / 104.0;
    if radius < 1.0 {
        return;
    }
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let x = x as f32 + 0.5;
        let y = y as f32 + 0.5;
        let center_x = x.clamp(radius, width as f32 - radius);
        let center_y = y.clamp(radius, height as f32 - radius);
        let distance = ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt();
        let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
        pixel[3] = (pixel[3] as f32 * coverage).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn album_corners_are_transparent_with_an_antialiased_edge() {
        let mut image = image::RgbaImage::from_pixel(208, 208, image::Rgba([200, 100, 50, 255]));
        super::round_corners(&mut image);
        for (x, y) in [(0, 0), (207, 0), (0, 207), (207, 207)] {
            assert_eq!(image.get_pixel(x, y)[3], 0);
        }
        assert_eq!(image.get_pixel(104, 104).0, [200, 100, 50, 255]);
        assert_eq!(image.get_pixel(104, 0)[3], 255);
        assert!(image.pixels().any(|p| p[3] > 0 && p[3] < 255));
    }
}
