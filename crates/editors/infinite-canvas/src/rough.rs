use std::f32::consts::{PI, TAU};

use block_editor_beui::beui::{Path, Pos2, Vec2, pos2};

const BOWING: f32 = 1.0;
const MAX_OFFSET: f32 = 2.0;
const CURVE_FITTING: f32 = 0.95;
const CURVE_STEPS: f32 = 9.0;
const MOST_HATCH_LINES: f32 = 2000.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Segment {
    Line(Pos2, Pos2),
    Cubic(Pos2, Pos2, Pos2, Pos2),
}

struct Random(u32);

impl Random {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        let state = self.0;
        let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
        (((word >> 22) ^ word) >> 8) as f32 / (1u32 << 24) as f32
    }
}

pub(crate) struct Rough {
    random: Random,
    roughness: f32,
    twice: bool,
}

impl Rough {
    pub(crate) fn new(seed: u32, roughness: f32, twice: bool) -> Self {
        Self {
            random: Random(seed),
            roughness: roughness.clamp(0.0, 10.0),
            twice,
        }
    }

    fn offset(&mut self, min: f32, max: f32, gain: f32) -> f32 {
        self.roughness * gain * (self.random.next() * (max - min) + min)
    }

    fn jitter(&mut self, reach: f32, gain: f32) -> f32 {
        self.offset(-reach, reach, gain)
    }

    fn moved(&mut self, at: Pos2, reach: f32, gain: f32) -> Pos2 {
        pos2(
            at.x + self.jitter(reach, gain),
            at.y + self.jitter(reach, gain),
        )
    }

    fn one_line(&mut self, path: Path, from: Pos2, to: Pos2, overlay: bool) -> Path {
        let length = (to - from).length();
        let gain = match length {
            length if length < 200.0 => 1.0,
            length if length > 500.0 => 0.4,
            length => -0.001_666_8 * length + 1.233_334,
        };
        let mut reach = MAX_OFFSET;
        if reach * reach * 100.0 > length * length {
            reach = length / 10.0;
        }
        let reach = match overlay {
            true => reach / 2.0,
            false => reach,
        };
        let diverge = 0.2 + self.random.next() * 0.2;
        let bow = Vec2::new(
            BOWING * MAX_OFFSET * (to.y - from.y) / 200.0,
            BOWING * MAX_OFFSET * (from.x - to.x) / 200.0,
        );
        let bow = Vec2::new(self.jitter(bow.x, gain), self.jitter(bow.y, gain));
        let start = self.moved(from, reach, gain);
        let along = to - from;
        let first = self.moved(from + bow + along * diverge, reach, gain);
        let second = self.moved(from + bow + along * (2.0 * diverge), reach, gain);
        let end = self.moved(to, reach, gain);
        path.move_to(start).cubic_to(first, second, end)
    }

    pub(crate) fn line(&mut self, path: Path, from: Pos2, to: Pos2) -> Path {
        let path = self.one_line(path, from, to, false);
        match self.twice {
            true => self.one_line(path, from, to, true),
            false => path,
        }
    }

    fn bezier(&mut self, mut path: Path, from: Pos2, first: Pos2, second: Pos2, to: Pos2) -> Path {
        let reaches = [MAX_OFFSET.max(1.0), MAX_OFFSET.max(1.0) + 0.3];
        let passes = if self.twice { 2 } else { 1 };
        for (pass, reach) in reaches.into_iter().enumerate().take(passes) {
            let start = match pass {
                0 => from,
                _ => self.moved(from, reaches[0], 1.0),
            };
            let first = self.moved(first, reach, 1.0);
            let second = self.moved(second, reach, 1.0);
            let end = self.moved(to, reach, 1.0);
            path = path.move_to(start).cubic_to(first, second, end);
        }
        path
    }

    pub(crate) fn segments(&mut self, segments: &[Segment]) -> Path {
        segments
            .iter()
            .fold(Path::new(), |path, segment| match *segment {
                Segment::Line(from, to) => self.line(path, from, to),
                Segment::Cubic(from, first, second, to) => {
                    self.bezier(path, from, first, second, to)
                }
            })
    }

    pub(crate) fn lines(&mut self, lines: &[[Pos2; 2]]) -> Path {
        lines
            .iter()
            .fold(Path::new(), |path, [from, to]| self.line(path, *from, *to))
    }

    pub(crate) fn solid(&mut self, points: &[Pos2]) -> Path {
        Path::polygon(
            points
                .iter()
                .map(|at| self.moved(*at, MAX_OFFSET, 1.0))
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn ellipse(&mut self, center: Pos2, radii: Vec2, rotation: f32) -> Ellipse {
        let (sin, cos) = rotation.sin_cos();
        let place = |at: Pos2| {
            pos2(
                center.x + at.x * cos - at.y * sin,
                center.y + at.x * sin + at.y * cos,
            )
        };
        let perimeter = (TAU * ((radii.x * radii.x + radii.y * radii.y) / 2.0).sqrt())
            .sqrt()
            .max(0.0);
        let steps = CURVE_STEPS
            .max(CURVE_STEPS / 200f32.sqrt() * perimeter)
            .ceil();
        let step = TAU / steps;
        let loose = 1.0 - CURVE_FITTING;
        let radii = Vec2::new(
            radii.x.abs() + self.jitter(radii.x.abs() * loose, 1.0),
            radii.y.abs() + self.jitter(radii.y.abs() * loose, 1.0),
        );
        let most = self.offset(0.4, 1.0, 1.0);
        let overlap = step * self.offset(0.1, most, 1.0);
        let (first, core) = self.ellipse_points(step, radii, 1.0, overlap);
        let first: Vec<Pos2> = first.into_iter().map(place).collect();
        let mut stroke = curve(Path::new(), &first);
        if self.twice {
            let (second, _) = self.ellipse_points(step, radii, 1.5, 0.0);
            let second: Vec<Pos2> = second.into_iter().map(place).collect();
            stroke = curve(stroke, &second);
        }
        Ellipse {
            stroke,
            fill: curve(Path::new(), &first).close(),
            outline: core.into_iter().map(place).collect(),
        }
    }

    fn ellipse_points(
        &mut self,
        step: f32,
        radii: Vec2,
        reach: f32,
        overlap: f32,
    ) -> (Vec<Pos2>, Vec<Pos2>) {
        let around = |angle: f32, shrink: f32, rough: &mut Self| {
            pos2(
                rough.jitter(reach, 1.0) + shrink * radii.x * angle.cos(),
                rough.jitter(reach, 1.0) + shrink * radii.y * angle.sin(),
            )
        };
        let start = self.jitter(0.5, 1.0) - PI / 2.0;
        let mut all = vec![around(start - step, 0.9, self)];
        let mut core = Vec::new();
        let mut angle = start;
        while angle < TAU + start - 0.01 {
            let at = around(angle, 1.0, self);
            core.push(at);
            all.push(at);
            angle += step;
        }
        all.push(around(start + TAU + overlap * 0.5, 1.0, self));
        all.push(around(start + overlap, 0.98, self));
        all.push(around(start + overlap * 0.5, 0.9, self));
        (all, core)
    }
}

pub(crate) struct Ellipse {
    pub(crate) stroke: Path,
    pub(crate) fill: Path,
    pub(crate) outline: Vec<Pos2>,
}

fn curve(path: Path, points: &[Pos2]) -> Path {
    match points {
        [] | [_] => path,
        [from, to] => path.move_to(*from).line_to(*to),
        [_, from, to] => path.move_to(*from).cubic_to(*from, *to, *to),
        _ => {
            let mut path = path.move_to(points[1]);
            for at in 1..points.len() - 2 {
                let (before, here, next, after) =
                    (points[at - 1], points[at], points[at + 1], points[at + 2]);
                path = path.cubic_to(
                    here + (next - before) / 6.0,
                    next + (here - after) / 6.0,
                    next,
                );
            }
            path
        }
    }
}

pub(crate) fn hachure(polygon: &[Pos2], gap: f32, angle: f32) -> Vec<[Pos2; 2]> {
    if polygon.len() < 3 || gap.is_nan() || gap <= 0.0 {
        return Vec::new();
    }
    let (sin, cos) = angle.sin_cos();
    let turned: Vec<Pos2> = polygon
        .iter()
        .map(|at| pos2(at.x * cos + at.y * sin, -at.x * sin + at.y * cos))
        .collect();
    let back = |x: f32, y: f32| pos2(x * cos - y * sin, x * sin + y * cos);
    let low = turned.iter().map(|at| at.y).fold(f32::INFINITY, f32::min);
    let high = turned
        .iter()
        .map(|at| at.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let gap = gap.max((high - low) / MOST_HATCH_LINES);
    let mut lines = Vec::new();
    let mut y = low + ((high - low) % gap) / 2.0;
    if y <= low {
        y += gap;
    }
    while y < high {
        let mut crossings: Vec<f32> = (0..turned.len())
            .filter_map(|at| {
                let (from, to) = (turned[at], turned[(at + 1) % turned.len()]);
                let crosses = (from.y <= y && to.y > y) || (to.y <= y && from.y > y);
                crosses.then(|| from.x + (y - from.y) / (to.y - from.y) * (to.x - from.x))
            })
            .collect();
        crossings.sort_by(f32::total_cmp);
        for pair in crossings.as_chunks::<2>().0 {
            lines.push([back(pair[0], y), back(pair[1], y)]);
        }
        y += gap;
    }
    lines
}
