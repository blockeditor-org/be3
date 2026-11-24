use skrifa::outline::OutlinePen;
use zeno::{Command, Mask, Placement, Point, Scratch};

/// A glyph's outline as skrifa draws it: in pixels, y up, one contour after another.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Outline {
    contours: Vec<Contour>,
}

#[derive(Clone, Debug, PartialEq)]
struct Contour {
    start: [f32; 2],
    segments: Vec<Segment>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Segment {
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
}

/// How an outline is placed into its raster: emboldened by `strength` pixels, sheared by
/// `shear` for a synthetic italic, both at the layout size as FreeType does, then scaled to
/// the raster and moved right by the subpixel `shift`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placed {
    pub(crate) strength: f32,
    pub(crate) shear: f32,
    pub(crate) scale: f32,
    pub(crate) shift: f32,
}

impl Outline {
    pub(crate) fn place(mut self, placed: Placed) -> Self {
        if placed.strength > 0.0 {
            self.embolden(placed.strength);
        }
        let shift = placed.shift;
        self.map(|[x, y]| {
            [
                (x + placed.shear * y) * placed.scale + shift,
                y * placed.scale,
            ]
        });
        self
    }

    fn map(&mut self, mut transform: impl FnMut([f32; 2]) -> [f32; 2]) {
        for contour in &mut self.contours {
            contour.start = transform(contour.start);
            for segment in &mut contour.segments {
                match segment {
                    Segment::Line(point) => *point = transform(*point),
                    Segment::Quad(control, point) => {
                        *control = transform(*control);
                        *point = transform(*point);
                    }
                    Segment::Cubic(first, second, point) => {
                        *first = transform(*first);
                        *second = transform(*second);
                        *point = transform(*point);
                    }
                }
            }
        }
    }

    /// The coverage of the outline, filled with the non-zero rule into an 8-bit mask, and
    /// where the mask sits: `top` counts down from the baseline, as the mask's rows do.
    pub(crate) fn fill(&self, scratch: &mut Scratch) -> (Vec<u8>, Placement) {
        let commands = self.commands();
        if commands.is_empty() {
            return (Vec::new(), Placement::default());
        }
        Mask::with_scratch(&commands, scratch).render()
    }

    fn commands(&self) -> Vec<Command> {
        let point = |[x, y]: [f32; 2]| Point::new(x, -y);
        let mut commands = Vec::new();
        for contour in &self.contours {
            commands.push(Command::MoveTo(point(contour.start)));
            for segment in &contour.segments {
                commands.push(match *segment {
                    Segment::Line(end) => Command::LineTo(point(end)),
                    Segment::Quad(control, end) => Command::QuadTo(point(control), point(end)),
                    Segment::Cubic(first, second, end) => {
                        Command::CurveTo(point(first), point(second), point(end))
                    }
                });
            }
            commands.push(Command::Close);
        }
        commands
    }

    /// Makes the outline `strength` pixels wider and taller, keeping its left side bearing
    /// and baseline: each point moves out from the ink along the bisector of its two edges
    /// by half the strength over one plus the cosine of the angle between them, no further
    /// than its shorter edge allows where the outline turns sharply inward, and the whole
    /// outline then moves up and right by half the strength. Which side is out follows the
    /// outline's orientation, so a counter shrinks as its ink grows.
    pub(crate) fn embolden(&mut self, strength: f32) {
        let half = strength / 2.0;
        let clockwise = self.area() < 0.0;
        for contour in &mut self.contours {
            let points = contour.points();
            let count = points.len();
            if count < 2 {
                continue;
            }
            let mut moved = Vec::with_capacity(count);
            for at in 0..count {
                let current = points[at];
                let neighbour = |step: isize| {
                    (1..count).find_map(|distance| {
                        let index = (at as isize + step * distance as isize)
                            .rem_euclid(count as isize) as usize;
                        let other = points[index];
                        let length = (other[0] - current[0]).hypot(other[1] - current[1]);
                        (length > f32::EPSILON).then_some((other, length))
                    })
                };
                let (Some((previous, before)), Some((next, after))) = (neighbour(-1), neighbour(1))
                else {
                    moved.push([current[0] + half, current[1] + half]);
                    continue;
                };
                let incoming = [
                    (current[0] - previous[0]) / before,
                    (current[1] - previous[1]) / before,
                ];
                let outgoing = [(next[0] - current[0]) / after, (next[1] - current[1]) / after];
                let along = 1.0 + incoming[0] * outgoing[0] + incoming[1] * outgoing[1];
                let mut shift = match clockwise {
                    true => [-(incoming[1] + outgoing[1]), incoming[0] + outgoing[0]],
                    false => [incoming[1] + outgoing[1], -(incoming[0] + outgoing[0])],
                };
                // Only a turn of less than about 160 degrees moves its point: a spike
                // sharper than that would be pushed far past its edges.
                if along > 1.0 / 16.0 {
                    let cross = incoming[0] * outgoing[1] - incoming[1] * outgoing[0];
                    let inward = if clockwise { cross } else { -cross };
                    let shortest = before.min(after);
                    let factor = match half * inward <= shortest * along {
                        true => half / along,
                        false => shortest / inward,
                    };
                    shift = [shift[0] * factor, shift[1] * factor];
                } else {
                    shift = [0.0, 0.0];
                }
                moved.push([current[0] + half + shift[0], current[1] + half + shift[1]]);
            }
            contour.set_points(&moved);
        }
    }

    /// Twice the signed area the outline encloses, over all its points: positive when its
    /// outer contours run counter-clockwise (y up), as PostScript outlines do, negative for
    /// TrueType's clockwise ones.
    fn area(&self) -> f32 {
        self.contours
            .iter()
            .map(|contour| {
                let points = contour.points();
                (0..points.len())
                    .map(|at| {
                        let [x0, y0] = points[at];
                        let [x1, y1] = points[(at + 1) % points.len()];
                        x0 * y1 - x1 * y0
                    })
                    .sum::<f32>()
            })
            .sum()
    }
}

impl Contour {
    fn points(&self) -> Vec<[f32; 2]> {
        let mut points = vec![self.start];
        for segment in &self.segments {
            match *segment {
                Segment::Line(end) => points.push(end),
                Segment::Quad(control, end) => points.extend([control, end]),
                Segment::Cubic(first, second, end) => points.extend([first, second, end]),
            }
        }
        if points.len() > 1 && points.last() == Some(&self.start) {
            points.pop();
        }
        points
    }

    fn set_points(&mut self, points: &[[f32; 2]]) {
        let count = points.len();
        let mut at = 0;
        let mut next = || {
            at += 1;
            points[at % count]
        };
        self.start = points[0];
        for segment in &mut self.segments {
            match segment {
                Segment::Line(end) => *end = next(),
                Segment::Quad(control, end) => {
                    *control = next();
                    *end = next();
                }
                Segment::Cubic(first, second, end) => {
                    *first = next();
                    *second = next();
                    *end = next();
                }
            }
        }
    }
}

impl OutlinePen for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(Contour {
            start: [x, y],
            segments: Vec::new(),
        });
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour.segments.push(Segment::Line([x, y]));
        }
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour.segments.push(Segment::Quad([cx0, cy0], [x, y]));
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour
                .segments
                .push(Segment::Cubic([cx0, cy0], [cx1, cy1], [x, y]));
        }
    }

    fn close(&mut self) {}
}

#[cfg(test)]
pub(crate) fn polygon(contours: &[&[[f32; 2]]]) -> Outline {
    let mut outline = Outline::default();
    for points in contours {
        outline.move_to(points[0][0], points[0][1]);
        for point in &points[1..] {
            outline.line_to(point[0], point[1]);
        }
    }
    outline
}

#[cfg(test)]
impl Outline {
    pub(crate) fn corners(&self) -> Vec<Vec<[f32; 2]>> {
        self.contours.iter().map(Contour::points).collect()
    }
}
