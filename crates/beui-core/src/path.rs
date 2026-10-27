use std::cell::RefCell;
use std::rc::Rc;

use foldhash::HashMap;
use kurbo::Shape as _;

pub use beui_types::path::{FillRule, LineCap, LineJoin, Path, PathElement, PathStyle, Stroke};

use crate::geometry::{Pos2, Rect, pos2};

const TOLERANCE: f32 = 0.05;
const DOT: f64 = 0.001;

fn kurbo_path(path: &Path, scale: f32) -> kurbo::BezPath {
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

fn kurbo_stroke(stroke: &Stroke, scale: f32) -> kurbo::Stroke {
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PathTriangle {
    pub corners: [[f32; 2]; 3],
    pub edges: u8,
}

pub struct PathPaint {
    path: Path,
    style: PathStyle,
    seams: Vec<[Pos2; 2]>,
    bounds: Rect,
    mesh: RefCell<Option<(f32, Rc<[PathTriangle]>)>>,
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
            mesh: RefCell::new(None),
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

    pub fn mesh(&self, scale: f32) -> Rc<[PathTriangle]> {
        if let Some((held, mesh)) = self.mesh.borrow().as_ref()
            && *held == scale
        {
            return Rc::clone(mesh);
        }
        let mut triangles = tessellate(&self.path, &self.style, scale);
        if !self.seams.is_empty() {
            let seams: Vec<[Pos2; 2]> = self
                .seams
                .iter()
                .map(|seam| seam.map(|at| pos2(at.x * scale, at.y * scale)))
                .collect();
            unseam(&mut triangles, &seams);
        }
        let mesh: Rc<[PathTriangle]> = triangles.into();
        *self.mesh.borrow_mut() = Some((scale, Rc::clone(&mesh)));
        mesh
    }
}

fn unseam(triangles: &mut [PathTriangle], seams: &[[Pos2; 2]]) {
    const NEAR: f32 = 1e-3;
    let on = |at: [f32; 2], [from, to]: [Pos2; 2]| {
        let at = pos2(at[0], at[1]);
        let (along, from_at) = (to - from, at - from);
        let squared = along.x * along.x + along.y * along.y;
        let t = match squared > 0.0 {
            true => ((from_at.x * along.x + from_at.y * along.y) / squared).clamp(0.0, 1.0),
            false => 0.0,
        };
        (from + along * t - at).length() <= NEAR * (1.0 + squared.sqrt())
    };
    for triangle in triangles {
        for side in 0..3 {
            let (from, to) = (triangle.corners[side], triangle.corners[(side + 1) % 3]);
            if triangle.edges & (1 << side) != 0
                && seams.iter().any(|seam| on(from, *seam) && on(to, *seam))
            {
                triangle.edges &= !(1 << side);
            }
        }
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

pub fn tessellate(path: &Path, style: &PathStyle, scale: f32) -> Vec<PathTriangle> {
    if path.is_empty() || scale.is_nan() || scale <= 0.0 {
        return Vec::new();
    }
    let (outline, rule) = match style {
        PathStyle::Fill(rule) => (kurbo_path(path, scale), *rule),
        PathStyle::Stroke(stroke) => {
            if !stroke.width.is_finite() || stroke.width <= 0.0 {
                return Vec::new();
            }
            let outline = kurbo::stroke(
                kurbo_path(path, scale),
                &kurbo_stroke(stroke, scale),
                &kurbo::StrokeOpts::default(),
                f64::from(TOLERANCE),
            );
            (outline, FillRule::NonZero)
        }
    };
    fill(&outline, rule)
}

fn fill(outline: &kurbo::BezPath, rule: FillRule) -> Vec<PathTriangle> {
    use lyon_tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    };

    if outline.is_empty() || !outline.bounding_box().is_finite() {
        return Vec::new();
    }
    let at = |at: kurbo::Point| lyon_tessellation::math::point(at.x as f32, at.y as f32);
    let mut builder = lyon_tessellation::path::Path::builder();
    let mut open = false;
    for element in outline.elements() {
        match *element {
            kurbo::PathEl::MoveTo(to) => {
                if open {
                    builder.end(false);
                }
                builder.begin(at(to));
                open = true;
            }
            kurbo::PathEl::LineTo(to) if open => {
                builder.line_to(at(to));
            }
            kurbo::PathEl::QuadTo(control, to) if open => {
                builder.quadratic_bezier_to(at(control), at(to));
            }
            kurbo::PathEl::CurveTo(first, second, to) if open => {
                builder.cubic_bezier_to(at(first), at(second), at(to));
            }
            kurbo::PathEl::ClosePath if open => {
                builder.end(true);
                open = false;
            }
            _ => {}
        }
    }
    if open {
        builder.end(false);
    }
    let outline = builder.build();
    let options = FillOptions::tolerance(TOLERANCE).with_fill_rule(match rule {
        FillRule::NonZero => lyon_tessellation::FillRule::NonZero,
        FillRule::EvenOdd => lyon_tessellation::FillRule::EvenOdd,
    });
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let filled = FillTessellator::new().tessellate_path(
        &outline,
        &options,
        &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
            vertex.position().to_array()
        }),
    );
    if filled.is_err() {
        return Vec::new();
    }
    triangles(&buffers.vertices, &buffers.indices)
}

fn triangles(vertices: &[[f32; 2]], indices: &[u32]) -> Vec<PathTriangle> {
    let solid: Vec<[[f32; 2]; 3]> = indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|corners| [0, 1, 2].map(|corner| vertices[corners[corner] as usize]))
        .filter(|[a, b, c]| {
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            area != 0.0 && area.is_finite()
        })
        .collect();
    let edge = |from: [f32; 2], to: [f32; 2]| {
        let (from, to) = (from.map(f32::to_bits), to.map(f32::to_bits));
        match from <= to {
            true => (from, to),
            false => (to, from),
        }
    };
    let mut shared: HashMap<([u32; 2], [u32; 2]), u32> = HashMap::default();
    for corners in &solid {
        for side in 0..3 {
            *shared
                .entry(edge(corners[side], corners[(side + 1) % 3]))
                .or_default() += 1;
        }
    }
    solid
        .into_iter()
        .map(|corners| {
            let edges = (0..3).fold(0, |edges, side| {
                match shared[&edge(corners[side], corners[(side + 1) % 3])] {
                    1 => edges | (1 << side),
                    _ => edges,
                }
            });
            PathTriangle { corners, edges }
        })
        .collect()
}

#[cfg(test)]
mod tests;
