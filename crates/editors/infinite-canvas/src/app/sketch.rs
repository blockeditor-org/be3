use block_editor_beui::beui::native::core_painter;
use std::rc::Rc;

use block_editor_beui::beui::{
    Color32, FillRule, LineCap, LineJoin, Path, Pos2, Stroke, Vec2, native::PathPaint, pos2,
    reactive::Painter,
};
use infinite_canvas_types::{
    CanvasEntity, CanvasFill, CanvasLook, CanvasPathPoint, CanvasPoint, CanvasShape,
    CanvasStrokeStyle, CanvasStyle, CanvasTransform,
};

use crate::freehand;
use crate::geometry::{flattened, outline_points};
use crate::rough::{Rough, Segment, hachure};

use super::paint::{arrow_ends, arrow_wings, path_stroke};

const HATCH_ANGLE: f32 = -0.715_585;
const HATCH_GAP: f32 = 4.0;
const CORNER_HANDLE: f32 = 0.447_715_2;
const CORNER_STEPS: usize = 6;
const FILL_SEED: u32 = 0x9e37_79b9;
const ELLIPSE_HANDLE: f32 = 0.552_284_8;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Ink {
    Stroke,
    Fill,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Mark {
    Fill(Path),
    Stroke(Path, Stroke),
    Painted(Rc<PathPaint>, Vec2),
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Part {
    pub(crate) ink: Ink,
    pub(crate) mark: Mark,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct Sketch {
    pub(crate) parts: Vec<Part>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct SketchKey {
    shape: CanvasShape,
    transform: CanvasTransform,
    points: Vec<CanvasPathPoint>,
    pressures: Vec<f32>,
    style: CanvasStyle,
    drawing: bool,
}

impl SketchKey {
    pub(crate) fn of(entity: &CanvasEntity, drawing: bool) -> Self {
        Self {
            drawing,
            shape: entity.shape,
            transform: entity.transform,
            points: entity.points.clone(),
            pressures: entity.pressures.clone(),
            style: entity.style,
        }
    }
}

fn at(point: CanvasPoint) -> Pos2 {
    pos2(point.x, point.y)
}

fn round(width: f32) -> Stroke {
    Stroke::new(width).join(LineJoin::Round).cap(LineCap::Round)
}

impl Sketch {
    pub(crate) fn of(entity: &CanvasEntity, drawing: bool) -> Self {
        let style = entity.style;
        let hand = style.look == CanvasLook::HandDrawn;
        let mut sketch = Self::default();
        match entity.shape {
            CanvasShape::Freehand => {
                let points: Vec<Pos2> = entity
                    .world_points()
                    .iter()
                    .map(|point| at(point.at()))
                    .collect();
                sketch.freehand(&points, &entity.pressures, style.stroke_width, drawing);
            }
            CanvasShape::Rectangle | CanvasShape::Ellipse | CanvasShape::Diamond => {
                sketch.closed(entity, hand)
            }
            CanvasShape::Path if hand => sketch.path(entity),
            CanvasShape::Path => sketch.clean_path(entity),
            _ => {}
        }
        sketch
    }

    fn freehand(&mut self, points: &[Pos2], pressures: &[f32], width: f32, drawing: bool) {
        let outline = freehand::stroke(points, pressures, width.max(0.1), !drawing);
        self.push(Ink::Stroke, Mark::Fill(outline));
    }

    pub(crate) fn painted(pieces: Vec<Rc<PathPaint>>) -> Self {
        let mut sketch = Self::default();
        for piece in pieces {
            sketch.push(Ink::Stroke, Mark::Painted(piece, Vec2::ZERO));
        }
        sketch
    }

    fn push(&mut self, ink: Ink, mark: Mark) {
        let empty = match &mark {
            Mark::Fill(path) | Mark::Stroke(path, _) => path.is_empty(),
            Mark::Painted(paint, _) => paint.path().is_empty(),
        };
        if !empty {
            self.parts.push(Part { ink, mark });
        }
    }

    fn closed(&mut self, entity: &CanvasEntity, hand: bool) {
        let style = entity.style;
        let width = style.stroke_width.max(0.0);
        let twice = style.stroke_style == CanvasStrokeStyle::Solid;
        let mut rough = Rough::new(style.seed, style.roughness, twice);
        let mut filler = Rough::new(style.seed ^ FILL_SEED, style.roughness, true);
        let transform = entity.transform;
        let (outline, stroke, solid) = match entity.shape {
            CanvasShape::Ellipse => {
                let center = at(transform.center);
                let radii = Vec2::new(transform.size.x / 2.0, transform.size.y / 2.0);
                match hand {
                    true => {
                        let ellipse = rough.ellipse(center, radii, transform.rotation);
                        (ellipse.outline, ellipse.stroke, ellipse.fill)
                    }
                    false => (
                        outline_points(entity).into_iter().map(at).collect(),
                        Path::new(),
                        Path::new(),
                    ),
                }
            }
            _ => {
                let segments = edges(entity);
                let outline = flatten(&segments);
                match hand {
                    true => {
                        let solid = filler.solid(&outline);
                        (outline, rough.segments(&segments), solid)
                    }
                    false => (outline, Path::new(), Path::new()),
                }
            }
        };
        let clean = (!hand).then(|| clean_outline(entity));
        let solid = clean.clone().unwrap_or(solid);
        match style.fill {
            CanvasFill::None => {}
            CanvasFill::Solid => self.push(Ink::Fill, Mark::Fill(solid)),
            CanvasFill::Hatched => {
                let lines = hachure(&outline, (width * HATCH_GAP).max(2.0), HATCH_ANGLE);
                let path = match hand {
                    true => filler.lines(&lines),
                    false => lines.iter().fold(Path::new(), |path, [from, to]| {
                        path.move_to(*from).line_to(*to)
                    }),
                };
                self.push(Ink::Fill, Mark::Stroke(path, round((width / 2.0).max(0.5))));
            }
        }
        let stroke = clean.unwrap_or(stroke);
        self.push(
            Ink::Stroke,
            Mark::Stroke(stroke, path_stroke(style.stroke_style, width.max(0.1))),
        );
    }

    fn clean_path(&mut self, entity: &CanvasEntity) {
        let style = entity.style;
        let width = style.stroke_width.max(0.1);
        let world = entity.world_points();
        let mut path = Path::new();
        for (index, point) in world.iter().enumerate() {
            let to = at(point.at());
            path = match index.checked_sub(1).map(|before| world[before]) {
                None => path.move_to(to),
                Some(before) if before.handle_out.is_none() && point.handle_in.is_none() => {
                    path.line_to(to)
                }
                Some(before) => path.cubic_to(
                    at(before.handle_out.unwrap_or(before.at())),
                    at(point.handle_in.unwrap_or(point.at())),
                    to,
                ),
            };
        }
        self.push(
            Ink::Stroke,
            Mark::Stroke(path, path_stroke(style.stroke_style, width)),
        );
        let line: Vec<Pos2> = flattened(&world).into_iter().map(at).collect();
        for (tip, inward, size) in arrow_ends(&style, &line) {
            let path = arrow_wings(tip, inward, size)
                .into_iter()
                .fold(Path::new(), |path, wing| path.move_to(tip).line_to(wing));
            self.push(
                Ink::Stroke,
                Mark::Stroke(path, round(width.max(size * 0.25))),
            );
        }
    }

    fn path(&mut self, entity: &CanvasEntity) {
        let style = entity.style;
        let width = style.stroke_width.max(0.1);
        let twice = style.stroke_style == CanvasStrokeStyle::Solid;
        let mut rough = Rough::new(style.seed, style.roughness, twice);
        let world = entity.world_points();
        let segments: Vec<Segment> = world
            .windows(2)
            .map(|pair| {
                let (from, to) = (pair[0], pair[1]);
                match from.handle_out.is_none() && to.handle_in.is_none() {
                    true => Segment::Line(at(from.at()), at(to.at())),
                    false => Segment::Cubic(
                        at(from.at()),
                        at(from.handle_out.unwrap_or(from.at())),
                        at(to.handle_in.unwrap_or(to.at())),
                        at(to.at()),
                    ),
                }
            })
            .collect();
        self.push(
            Ink::Stroke,
            Mark::Stroke(
                rough.segments(&segments),
                path_stroke(style.stroke_style, width),
            ),
        );
        let line: Vec<Pos2> = flattened(&world).into_iter().map(at).collect();
        let mut heads = Rough::new(style.seed ^ FILL_SEED, style.roughness, true);
        for (tip, inward, size) in arrow_ends(&style, &line) {
            let [left, right] = arrow_wings(tip, inward, size);
            let path = heads.line(Path::new(), tip, left);
            let path = heads.line(path, tip, right);
            self.push(
                Ink::Stroke,
                Mark::Stroke(path, round(width.max(size * 0.25))),
            );
        }
    }

    pub(crate) fn translated(&self, by: Vec2) -> Self {
        Self {
            parts: self
                .parts
                .iter()
                .map(|part| Part {
                    ink: part.ink,
                    mark: match &part.mark {
                        Mark::Fill(path) => Mark::Fill(path.translated(by)),
                        Mark::Stroke(path, stroke) => {
                            Mark::Stroke(path.translated(by), stroke.clone())
                        }
                        Mark::Painted(paint, offset) => {
                            Mark::Painted(Rc::clone(paint), *offset + by)
                        }
                    },
                })
                .collect(),
        }
    }

    pub(crate) fn draw(&self, painter: &Painter, stroke: Color32, fill: Color32) {
        self.draw_only(painter, stroke, fill, |_| true);
    }

    pub(crate) fn draw_only(
        &self,
        painter: &Painter,
        stroke: Color32,
        fill: Color32,
        wanted: impl Fn(&Part) -> bool,
    ) {
        for part in self.parts.iter().filter(|part| wanted(part)) {
            let color = match part.ink {
                Ink::Stroke => stroke,
                Ink::Fill => fill,
            };
            match &part.mark {
                Mark::Fill(path) => painter.fill_path(path, FillRule::NonZero, color),
                Mark::Stroke(path, line) => painter.stroke_path(path, line, color),
                Mark::Painted(paint, offset) => core_painter(painter)
                    .transformed(*offset, 1.0)
                    .paint_path(paint, color),
            }
        }
    }
}

pub(crate) fn clean_outline(entity: &CanvasEntity) -> Path {
    let transform = entity.transform;
    let world = |x: f32, y: f32| at(transform.to_world(CanvasPoint::new(x, y)));
    match entity.shape {
        CanvasShape::Ellipse => {
            let handle = 0.5 * ELLIPSE_HANDLE;
            Path::new()
                .move_to(world(0.5, 0.0))
                .cubic_to(world(0.5, handle), world(handle, 0.5), world(0.0, 0.5))
                .cubic_to(world(-handle, 0.5), world(-0.5, handle), world(-0.5, 0.0))
                .cubic_to(world(-0.5, -handle), world(-handle, -0.5), world(0.0, -0.5))
                .cubic_to(world(handle, -0.5), world(0.5, -handle), world(0.5, 0.0))
                .close()
        }
        CanvasShape::Diamond => Path::polygon([
            world(0.0, -0.5),
            world(0.5, 0.0),
            world(0.0, 0.5),
            world(-0.5, 0.0),
        ]),
        _ if entity.style.corner_radius > 0.0 => {
            let segments = edges(entity);
            let start = match segments.first() {
                Some(Segment::Line(from, _) | Segment::Cubic(from, ..)) => *from,
                None => return Path::new(),
            };
            segments
                .iter()
                .fold(Path::new().move_to(start), |path, segment| match *segment {
                    Segment::Line(_, to) => path.line_to(to),
                    Segment::Cubic(_, first, second, to) => path.cubic_to(first, second, to),
                })
                .close()
        }
        _ => Path::polygon([
            world(-0.5, -0.5),
            world(0.5, -0.5),
            world(0.5, 0.5),
            world(-0.5, 0.5),
        ]),
    }
}

fn edges(entity: &CanvasEntity) -> Vec<Segment> {
    let transform = entity.transform;
    let world = |x: f32, y: f32| at(transform.to_world(CanvasPoint::new(x, y)));
    let radius = entity
        .style
        .corner_radius
        .max(0.0)
        .min(transform.size.x.abs() / 2.0)
        .min(transform.size.y.abs() / 2.0);
    let corners: Vec<Pos2> = outline_points(entity).into_iter().map(at).collect();
    if entity.shape != CanvasShape::Rectangle || radius <= 0.0 {
        return (0..corners.len())
            .map(|index| Segment::Line(corners[index], corners[(index + 1) % corners.len()]))
            .collect();
    }
    let (rx, ry) = (
        radius / transform.size.x.abs().max(f32::EPSILON),
        radius / transform.size.y.abs().max(f32::EPSILON),
    );
    let (hx, hy) = (rx * CORNER_HANDLE, ry * CORNER_HANDLE);
    let mut segments = Vec::new();
    for [sx, sy] in [[1.0, -1.0], [1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0]] {
        let clockwise = sx * sy < 0.0;
        let (from, first, second, to) = match clockwise {
            true => (
                (0.5 * sx - rx * sx, 0.5 * sy),
                (0.5 * sx - hx * sx, 0.5 * sy),
                (0.5 * sx, 0.5 * sy - hy * sy),
                (0.5 * sx, 0.5 * sy - ry * sy),
            ),
            false => (
                (0.5 * sx, 0.5 * sy - ry * sy),
                (0.5 * sx, 0.5 * sy - hy * sy),
                (0.5 * sx - hx * sx, 0.5 * sy),
                (0.5 * sx - rx * sx, 0.5 * sy),
            ),
        };
        if let Some(Segment::Cubic(_, _, _, end) | Segment::Line(_, end)) = segments.last() {
            segments.push(Segment::Line(*end, world(from.0, from.1)));
        }
        segments.push(Segment::Cubic(
            world(from.0, from.1),
            world(first.0, first.1),
            world(second.0, second.1),
            world(to.0, to.1),
        ));
    }
    if let (Some(Segment::Cubic(_, _, _, end)), Some(Segment::Cubic(start, ..))) =
        (segments.last(), segments.first())
    {
        segments.push(Segment::Line(*end, *start));
    }
    segments
}

fn flatten(segments: &[Segment]) -> Vec<Pos2> {
    let mut points = Vec::new();
    for segment in segments {
        match *segment {
            Segment::Line(from, _) => points.push(from),
            Segment::Cubic(from, first, second, to) => {
                for step in 0..CORNER_STEPS {
                    let t = step as f32 / CORNER_STEPS as f32;
                    let u = 1.0 - t;
                    let mix = |a: f32, b: f32, c: f32, d: f32| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    points.push(pos2(
                        mix(from.x, first.x, second.x, to.x),
                        mix(from.y, first.y, second.y, to.y),
                    ));
                }
            }
        }
    }
    points
}

pub(crate) fn rough_reach(entity: &CanvasEntity) -> f32 {
    let style = entity.style;
    let rough = matches!(
        entity.shape,
        CanvasShape::Rectangle | CanvasShape::Ellipse | CanvasShape::Diamond | CanvasShape::Path
    ) && style.look == CanvasLook::HandDrawn;
    match rough {
        true => {
            let size = entity.transform.size;
            style.roughness.clamp(0.0, 10.0) * (3.0 + 0.05 * size.x.abs().max(size.y.abs()))
        }
        false => 0.0,
    }
}
