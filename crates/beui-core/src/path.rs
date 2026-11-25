use beui_types::{LineCap, LineJoin, Path, PathElement, PathStyle, Stroke};

use beui_types::{Pos2, Rect};

const DOT: f64 = 0.001;

/// `path` as kurbo holds a path, its points scaled by `scale`.
pub fn kurbo_path(path: &Path, scale: f32) -> kurbo::BezPath {
    let point =
        |point: Pos2| kurbo::Point::new(f64::from(point.x * scale), f64::from(point.y * scale));
    let mut out = kurbo::BezPath::new();
    let mut start = Pos2::ZERO;
    let mut closed = false;
    for element in path.elements() {
        if closed && !matches!(element, PathElement::MoveTo(_) | PathElement::Close) {
            out.move_to(point(start));
        }
        closed = false;
        match *element {
            PathElement::MoveTo(to) => {
                out.move_to(point(to));
                start = to;
            }
            PathElement::LineTo(to) => out.line_to(point(to)),
            PathElement::QuadTo(control, to) => out.quad_to(point(control), point(to)),
            PathElement::CubicTo(first, second, to) => {
                out.curve_to(point(first), point(second), point(to))
            }
            PathElement::Close => {
                out.close_path();
                closed = true;
            }
        }
    }
    out
}

/// `stroke` as kurbo holds a stroke, its widths and dashes scaled by `scale`.
pub fn kurbo_stroke(stroke: &Stroke, scale: f32) -> kurbo::Stroke {
    let scaled = |value: f32| f64::from(value * scale);
    let out = kurbo::Stroke::new(scaled(stroke.width))
        .with_join(match stroke.join {
            LineJoin::Miter => kurbo::Join::Miter,
            LineJoin::Round => kurbo::Join::Round,
            LineJoin::Bevel => kurbo::Join::Bevel,
        })
        .with_miter_limit(f64::from(stroke.miter_limit.max(1.0)))
        .with_caps(match stroke.cap {
            LineCap::Butt => kurbo::Cap::Butt,
            LineCap::Round => kurbo::Cap::Round,
            LineCap::Square => kurbo::Cap::Square,
        });
    if !stroke.is_dashed() {
        return out;
    }
    let mut pattern: Vec<f64> = stroke.dashes.iter().map(|dash| scaled(*dash)).collect();
    if pattern.len() % 2 == 1 {
        pattern.extend_from_within(..);
    }
    for pair in pattern.as_chunks_mut::<2>().0 {
        let dot = DOT.min(pair[1]) - pair[0];
        if dot > 0.0 {
            pair[0] += dot;
            pair[1] -= dot;
        }
    }
    out.with_dashes(scaled(stroke.dash_offset), pattern)
}

pub struct PathPaint {
    path: Path,
    style: PathStyle,
    seams: Vec<[Pos2; 2]>,
    bounds: Rect,
}

impl PathPaint {
    pub fn new(path: Path, style: PathStyle) -> Self {
        Self::with_seams(path, style, Vec::new())
    }

    pub fn with_seams(path: Path, style: PathStyle, seams: Vec<[Pos2; 2]>) -> Self {
        let bounds = match path.is_empty() {
            true => Rect::NOTHING,
            false => path.bounds().expand(style.reach()),
        };
        Self {
            path,
            style,
            seams,
            bounds,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn style(&self) -> &PathStyle {
        &self.style
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Whether it meets other fills on seams: the renderer and the rasterizer fill the seamed
    /// nonzero fills that follow each other alike as one path, so their shared edges cancel.
    pub fn seamed(&self) -> bool {
        !self.seams.is_empty()
    }
}

impl PartialEq for PathPaint {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || (self.path == other.path && self.style == other.style && self.seams == other.seams)
    }
}

impl std::fmt::Debug for PathPaint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PathPaint")
            .field("path", &self.path)
            .field("style", &self.style)
            .finish()
    }
}
