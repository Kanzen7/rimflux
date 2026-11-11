use iced::{Point, Size, widget::canvas::Path};

/// Shared silhouette for Canvas, Win32 shaping, and Wayland input regions.
pub fn outline(size: Size) -> Vec<Point> {
    let (w, h) = (size.width, size.height);
    let shoulder = 13.0_f32.min(h * 0.4);
    let radius = 12.0_f32.min((h - shoulder) * 0.75);
    let mut points = vec![Point::ORIGIN];
    cubic(
        &mut points,
        [
            Point::ORIGIN,
            Point::new(shoulder, 0.0),
            Point::new(shoulder, shoulder * 0.45),
            Point::new(shoulder, shoulder),
        ],
    );
    points.push(Point::new(shoulder, h - radius));
    cubic(
        &mut points,
        [
            Point::new(shoulder, h - radius),
            Point::new(shoulder, h),
            Point::new(shoulder + radius * 0.5, h),
            Point::new(shoulder + radius, h),
        ],
    );
    points.push(Point::new(w - shoulder - radius, h));
    cubic(
        &mut points,
        [
            Point::new(w - shoulder - radius, h),
            Point::new(w - shoulder - radius * 0.5, h),
            Point::new(w - shoulder, h),
            Point::new(w - shoulder, h - radius),
        ],
    );
    points.push(Point::new(w - shoulder, shoulder));
    cubic(
        &mut points,
        [
            Point::new(w - shoulder, shoulder),
            Point::new(w - shoulder, shoulder * 0.45),
            Point::new(w - shoulder, 0.0),
            Point::new(w, 0.0),
        ],
    );
    points
}

fn cubic(points: &mut Vec<Point>, p: [Point; 4]) {
    for step in 1..=16 {
        let t = step as f32 / 16.0;
        let a = 1.0 - t;
        points.push(Point::new(
            a.powi(3) * p[0].x
                + 3.0 * a * a * t * p[1].x
                + 3.0 * a * t * t * p[2].x
                + t.powi(3) * p[3].x,
            a.powi(3) * p[0].y
                + 3.0 * a * a * t * p[1].y
                + 3.0 * a * t * t * p[2].y
                + t.powi(3) * p[3].y,
        ));
    }
}

pub fn path(size: Size) -> Path {
    let points = outline(size);
    Path::new(|p| {
        p.move_to(points[0]);
        for point in &points[1..] {
            p.line_to(*point);
        }
        p.close();
    })
}

/// Horizontal intersection of the symmetric silhouette at a given logical y.
pub fn span(size: Size, y: f32) -> Option<(f32, f32)> {
    if y < 0.0 || y >= size.height {
        return None;
    }
    let points = outline(size);
    let mut left = size.width;
    let mut right = 0.0_f32;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        if (a.y > y) != (b.y > y) {
            let x = a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x);
            left = left.min(x);
            right = right.max(x);
        }
    }
    (left <= right).then_some((left, right))
}

pub fn contains(size: Size, point: Point) -> bool {
    span(size, point.y).is_some_and(|(left, right)| point.x >= left && point.x <= right)
}
