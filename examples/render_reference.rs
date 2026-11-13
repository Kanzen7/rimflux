//! Development-only SVG rasterization for visual reference inspection.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let data = std::fs::read(args.get(1).ok_or("input SVG required")?)?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width() * 3, size.height() * 3)
        .ok_or("invalid SVG size")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(3.0, 3.0),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(args.get(2).ok_or("output PNG required")?)?;
    Ok(())
}
