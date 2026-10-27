use std::f32::consts::PI;
use std::time::Instant;

use std::rc::Rc;

use block_editor_beui::beui::{FillRule, Path, PathStyle, Pos2, Vec2, native::PathPaint, pos2};

const HALF_TURN: f32 = PI + 0.0001;
const RESTING_PRESSURE: f32 = 0.5;
const THINNING: f32 = 0.75;
const SIMULATED_THINNING: f32 = 0.5;
const FAST: f32 = 1.0;
const SPEED_LAG: f32 = 24.0;
const PRESSURE_LAG: f32 = 52.0;
const SMOOTHING: f32 = 0.5;
const STREAMLINE: f32 = 0.5;
const TAPER: f32 = 3.0;
const TAPER_STRIDE: f32 = 0.5;
const FLATNESS: f32 = 0.02;
const MOST_STEPS: usize = 64;
const END_NOISE: f32 = 0.2;
const DOT_STEPS: usize = 16;

#[derive(Clone, Copy)]
struct StrokePoint {
    at: Pos2,
    pressure: f32,
    toward: Vec2,
    run: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ink {
    at: Pos2,
    time: Instant,
    speed: f32,
    pressure: f32,
}

impl Ink {
    pub(crate) fn new(at: Pos2, time: Instant) -> Self {
        Self {
            at,
            time,
            speed: 0.0,
            pressure: RESTING_PRESSURE,
        }
    }

    pub(crate) fn pressure(&self) -> f32 {
        RESTING_PRESSURE + (self.pressure - RESTING_PRESSURE) * SIMULATED_THINNING / THINNING
    }

    pub(crate) fn sample(&mut self, at: Pos2, time: Instant) -> f32 {
        let elapsed = time.saturating_duration_since(self.time).as_secs_f32() * 1000.0;
        if elapsed <= 0.0 {
            return self.pressure();
        }
        let measured = (at - self.at).length() / elapsed;
        let before = self.speed;
        self.speed += (measured - before) * (1.0 - (-elapsed / SPEED_LAG).exp());
        let speed = ((before + self.speed) / 2.0 / FAST).min(1.0);
        let rate = 1.0 - (-elapsed * speed / PRESSURE_LAG).exp();
        self.pressure += (1.0 - speed - self.pressure) * rate;
        self.at = at;
        self.time = time;
        self.pressure()
    }
}

pub(crate) fn stroke(points: &[Pos2], pressures: &[f32], size: f32, last: bool) -> Path {
    if size <= 0.0 || size.is_nan() {
        return Path::new();
    }
    let points = splined(&streamlined(raw(points, pressures), size, last), size);
    smooth_closed(&outline(&points, size, THINNING, last))
}

fn raw(points: &[Pos2], pressures: &[f32]) -> Vec<(Pos2, f32)> {
    let pressure_at = |at: usize| {
        pressures
            .get(at)
            .or(pressures.last())
            .copied()
            .unwrap_or(RESTING_PRESSURE)
            .clamp(0.0, 1.0)
    };
    points
        .iter()
        .enumerate()
        .map(|(at, point)| (*point, pressure_at(at)))
        .collect()
}

fn middle(a: Pos2, b: Pos2) -> Pos2 {
    a + (b - a) * 0.5
}

fn smooth_closed(outline: &[Pos2]) -> Path {
    let (Some(&first), Some(&end)) = (outline.first(), outline.last()) else {
        return Path::new();
    };
    if outline.len() < 3 {
        return Path::new();
    }
    outline
        .iter()
        .enumerate()
        .fold(
            Path::new().move_to(middle(end, first)),
            |path, (at, point)| {
                let next = outline[(at + 1) % outline.len()];
                path.quad_to(*point, middle(*point, next))
            },
        )
        .close()
}

fn unit(vector: Vec2) -> Vec2 {
    let length = vector.length();
    match length > f32::EPSILON {
        true => vector / length,
        false => Vec2::ZERO,
    }
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn disc(center: Pos2, radius: f32) -> Vec<Pos2> {
    (0..DOT_STEPS)
        .map(|step| {
            let angle = step as f32 / DOT_STEPS as f32 * 2.0 * PI;
            pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

fn across(vector: Vec2) -> Vec2 {
    Vec2::new(vector.y, -vector.x)
}

fn lerp(from: Vec2, to: Vec2, t: f32) -> Vec2 {
    from + (to - from) * t
}

fn around(point: Pos2, center: Pos2, angle: f32) -> Pos2 {
    let (sin, cos) = angle.sin_cos();
    let relative = point - center;
    pos2(
        center.x + relative.x * cos - relative.y * sin,
        center.y + relative.x * sin + relative.y * cos,
    )
}

#[derive(Clone)]
struct Streaming {
    out: Vec<(Pos2, f32)>,
    run: f32,
    long_enough: bool,
}

impl Streaming {
    fn new(first: (Pos2, f32)) -> Self {
        Self {
            out: vec![first],
            run: 0.0,
            long_enough: false,
        }
    }

    fn next(&self, point: Pos2, size: f32, kept: bool) -> Option<Pos2> {
        let before = self.out[self.out.len() - 1].0;
        let lag = 1.0 - (0.15 + (1.0 - STREAMLINE) * 0.85);
        let at = match kept {
            true => point,
            false => before + (point - before) * (1.0 - lag.powf((point - before).length() / size)),
        };
        (at != before).then_some(at)
    }

    fn take(&mut self, (point, pressure): (Pos2, f32), size: f32, end: bool, last: bool) -> bool {
        let before = self.out[self.out.len() - 1].0;
        let Some(at) = self.next(point, size, last && end) else {
            return false;
        };
        self.run += (at - before).length();
        if !end && !self.long_enough {
            if self.run < size {
                return false;
            }
            self.long_enough = true;
        }
        self.out.push((at, pressure));
        true
    }
}

fn stroke_point((at, pressure): (Pos2, f32)) -> StrokePoint {
    StrokePoint {
        at,
        pressure,
        toward: Vec2::ZERO,
        run: 0.0,
    }
}

fn streamlined(mut raw: Vec<(Pos2, f32)>, size: f32, last: bool) -> Vec<StrokePoint> {
    let Some(&(first, first_pressure)) = raw.first() else {
        return Vec::new();
    };
    if let [_, (end, pressure)] = raw[..] {
        raw.truncate(1);
        for step in 1..5 {
            let t = step as f32 / 4.0;
            raw.push((
                first + (end - first) * t,
                first_pressure + (pressure - first_pressure) * t,
            ));
        }
    }
    let mut streaming = Streaming::new((first, first_pressure));
    let end = raw.len() - 1;
    for (index, point) in raw.into_iter().enumerate().skip(1) {
        streaming.take(point, size, index == end, last);
    }
    streaming.out.into_iter().map(stroke_point).collect()
}

fn spline_at(get: impl Fn(usize) -> Pos2, last: isize, index: isize) -> Pos2 {
    match index {
        -1 if last > 0 => get(0) + (get(0) - get(1)),
        index if index > last && last > 0 => {
            let end = get(last as usize);
            end + (end - get(last as usize - 1))
        }
        index => get(index.clamp(0, last) as usize),
    }
}

struct Segment {
    gap: f32,
    tapered: bool,
}

impl Segment {
    fn of(from: Pos2, to: Pos2, along: f32, total: f32, size: f32) -> Self {
        let taper = size * TAPER;
        let gap = (to - from).length();
        Self {
            gap,
            tapered: along < taper || along + gap > total - taper,
        }
    }

    fn settled(&self, along: f32, size: f32) -> bool {
        along < size * TAPER || !self.tapered
    }
}

fn spline_segment(
    at: impl Fn(isize) -> Pos2,
    index: usize,
    pressures: (f32, f32),
    segment: &Segment,
    size: f32,
    out: &mut Vec<StrokePoint>,
) {
    let knot = |a: Pos2, b: Pos2| (b - a).length().sqrt().max(f32::EPSILON);
    let index = index as isize;
    let (p0, p1, p2, p3) = (at(index - 1), at(index), at(index + 1), at(index + 2));
    let (d0, d1, d2) = (knot(p0, p1), knot(p1, p2), knot(p2, p3));
    let leave = ((p1 - p0) / d0 - (p2 - p0) / (d0 + d1) + (p2 - p1) / d1) * d1;
    let arrive = ((p2 - p1) / d1 - (p3 - p1) / (d1 + d2) + (p3 - p2) / d2) * d1;
    let (c1, c2) = (p1 + leave / 3.0, p2 - arrive / 3.0);
    let chord = unit(p2 - p1);
    let bend = cross(c1 - p1, chord).abs().max(cross(c2 - p1, chord).abs());
    let stride = match segment.tapered {
        true => segment.gap / (size * TAPER_STRIDE),
        false => 1.0,
    };
    let steps = stride.max((bend / (size * FLATNESS)).sqrt()).ceil() as usize;
    let steps = steps.clamp(1, MOST_STEPS);
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let u = 1.0 - t;
        let curve = |a: f32, b: f32, c: f32, d: f32| {
            u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
        };
        out.push(StrokePoint {
            at: pos2(curve(p1.x, c1.x, c2.x, p2.x), curve(p1.y, c1.y, c2.y, p2.y)),
            pressure: pressures.0 + (pressures.1 - pressures.0) * t,
            toward: Vec2::ZERO,
            run: 0.0,
        });
    }
}

fn measure(before: Option<StrokePoint>, out: &mut [StrokePoint]) {
    let mut previous = before;
    for point in out.iter_mut() {
        if let Some(previous) = previous {
            let step = previous.at - point.at;
            point.run = previous.run + step.length();
            point.toward = unit(step);
        }
        previous = Some(*point);
    }
}

fn splined(points: &[StrokePoint], size: f32) -> Vec<StrokePoint> {
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    let last = points.len() as isize - 1;
    let at = |index: isize| spline_at(|at| points[at].at, last, index);
    let total: f32 = points
        .windows(2)
        .map(|pair| (pair[1].at - pair[0].at).length())
        .sum();
    let mut along = 0.0;
    let mut out = vec![first];
    for (index, pair) in points.windows(2).enumerate() {
        let segment = Segment::of(pair[0].at, pair[1].at, along, total, size);
        spline_segment(
            at,
            index,
            (pair[0].pressure, pair[1].pressure),
            &segment,
            size,
            &mut out,
        );
        along += segment.gap;
    }
    measure(None, &mut out);
    out[0].toward = out.get(1).map_or(Vec2::ZERO, |next| next.toward);
    out
}

fn radius_of(pressure: f32, size: f32, thinning: f32) -> f32 {
    size * (0.5 - thinning * (0.5 - pressure))
}

#[derive(Clone, Copy)]
struct Edges {
    radius: f32,
    before_toward: Vec2,
    held_left: Pos2,
    held_right: Pos2,
    sharp_before: bool,
}

struct Sides<'a> {
    left: &'a mut Vec<Pos2>,
    right: &'a mut Vec<Pos2>,
}

impl Edges {
    fn new(first: StrokePoint, radius: f32) -> Self {
        Self {
            radius,
            before_toward: first.toward,
            held_left: first.at,
            held_right: first.at,
            sharp_before: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        index: usize,
        point: StrokePoint,
        next: Option<StrokePoint>,
        total: f32,
        size: f32,
        thinning: f32,
        last: bool,
        sides: Sides<'_>,
    ) {
        let is_last = next.is_none();
        if !is_last && total - point.run < size * END_NOISE {
            return;
        }
        let taper = size * TAPER;
        let starting = match point.run < taper {
            true => {
                let t = point.run / taper;
                t * (2.0 - t)
            }
            false => 1.0,
        };
        let ending = match last && total - point.run < taper {
            true => {
                let t = (total - point.run) / taper - 1.0;
                t * t * t + 1.0
            }
            false => 1.0,
        };
        let radius =
            (radius_of(point.pressure, size, thinning) * starting.min(ending)).max(size * 0.005);
        self.radius = radius;
        let next_toward = next.map_or(point.toward, |next| next.toward);
        let next_turn = match is_last {
            true => 1.0,
            false => dot(point.toward, next_toward),
        };
        let turn = dot(point.toward, self.before_toward);
        let sharp = turn < 0.0 && !self.sharp_before;
        let sharp_next = next_turn < 0.0;
        if sharp || sharp_next {
            let offset = across(self.before_toward) * radius;
            for step in 0..=13 {
                let t = step as f32 / 13.0;
                self.held_left = around(point.at - offset, point.at, HALF_TURN * t);
                sides.left.push(self.held_left);
                self.held_right = around(point.at + offset, point.at, -HALF_TURN * t);
                sides.right.push(self.held_right);
            }
            self.sharp_before = sharp_next;
            return;
        }
        self.sharp_before = false;
        if is_last {
            let offset = across(point.toward) * radius;
            sides.left.push(point.at - offset);
            sides.right.push(point.at + offset);
            return;
        }
        let least = (size * SMOOTHING).powi(2);
        let offset = across(lerp(next_toward, point.toward, next_turn)) * radius;
        let at_left = point.at - offset;
        if index <= 1 || dot(self.held_left - at_left, self.held_left - at_left) > least {
            sides.left.push(at_left);
            self.held_left = at_left;
        }
        let at_right = point.at + offset;
        if index <= 1 || dot(self.held_right - at_right, self.held_right - at_right) > least {
            sides.right.push(at_right);
            self.held_right = at_right;
        }
        self.before_toward = point.toward;
    }
}

fn end_cap(end: StrokePoint, radius: f32, last: bool) -> Vec<Pos2> {
    match last {
        true => vec![end.at],
        false => {
            let from = end.at + across(-end.toward) * radius;
            (1..29)
                .map(|step| around(from, end.at, HALF_TURN * 3.0 * step as f32 / 29.0))
                .collect()
        }
    }
}

fn dot_of(first: StrokePoint, size: f32, thinning: f32) -> Vec<Pos2> {
    disc(
        first.at,
        radius_of(first.pressure, size, thinning).max(size * 0.25),
    )
}

fn outline(points: &[StrokePoint], size: f32, thinning: f32, last: bool) -> Vec<Pos2> {
    let (Some(&first), Some(&end)) = (points.first(), points.last()) else {
        return Vec::new();
    };
    let total = end.run;
    if total < size || points.len() == 1 {
        return dot_of(first, size, thinning);
    }
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut edges = Edges::new(first, radius_of(end.pressure, size, thinning));
    for (index, point) in points.iter().enumerate() {
        let sides = Sides {
            left: &mut left,
            right: &mut right,
        };
        edges.visit(
            index,
            *point,
            points.get(index + 1).copied(),
            total,
            size,
            thinning,
            last,
            sides,
        );
    }
    if left.is_empty() || right.is_empty() {
        return dot_of(first, size, thinning);
    }
    right.reverse();
    left.into_iter()
        .chain(end_cap(end, edges.radius, last))
        .chain(right)
        .collect()
}

const PIECE: usize = 48;

pub(crate) struct LiveStroke {
    size: f32,
    raw: Vec<(Pos2, f32)>,
    streaming: Option<Streaming>,
    folded: usize,
    length: f32,
    spline: Vec<StrokePoint>,
    segments: usize,
    along: f32,
    edges: Option<Edges>,
    visited: usize,
    left: Vec<Pos2>,
    right: Vec<Pos2>,
    pieces: Vec<Rc<PathPaint>>,
    boundary: Option<(usize, usize)>,
}

impl LiveStroke {
    pub(crate) fn new(size: f32) -> Self {
        Self {
            size,
            raw: Vec::new(),
            streaming: None,
            folded: 0,
            length: 0.0,
            spline: Vec::new(),
            segments: 0,
            along: 0.0,
            edges: None,
            visited: 0,
            left: Vec::new(),
            right: Vec::new(),
            pieces: Vec::new(),
            boundary: None,
        }
    }

    pub(crate) fn size(&self) -> f32 {
        self.size
    }

    pub(crate) fn draw(&mut self, points: &[Pos2], pressures: &[f32]) -> Vec<Rc<PathPaint>> {
        let raw = raw(points, pressures);
        if !raw.starts_with(&self.raw) {
            *self = Self::new(self.size);
        }
        self.raw = raw;
        let fill = |path: Path, seams: Vec<[Pos2; 2]>| {
            Rc::new(PathPaint::with_seams(
                path,
                PathStyle::Fill(FillRule::NonZero),
                seams,
            ))
        };
        let size = self.size;
        if size <= 0.0 || size.is_nan() || self.raw.len() < 3 {
            return vec![fill(stroke(points, pressures, size, false), Vec::new())];
        }
        self.fold();
        let streaming = self.streaming.as_ref().expect("the stroke is folded");
        let newest = self.raw[self.raw.len() - 1];
        let provisional = streaming
            .next(newest.0, size, false)
            .map(|at| (at, newest.1));
        let stable = &streaming.out;
        let count = stable.len() + usize::from(provisional.is_some());
        let point = |at: usize| match at < stable.len() {
            true => stable[at],
            false => provisional.expect("the provisional point is last"),
        };
        let total = self.length
            + provisional.map_or(0.0, |(at, _)| (at - stable[stable.len() - 1].0).length());
        let last = count as isize - 1;
        let at = |index: isize| spline_at(|at| point(at).0, last, index);
        let settled = self.spline.len();
        while streaming.long_enough && self.segments + 2 < stable.len() {
            let index = self.segments;
            let segment = Segment::of(
                stable[index].0,
                stable[index + 1].0,
                self.along,
                total,
                size,
            );
            if !segment.settled(self.along, size) {
                break;
            }
            let pressures = (stable[index].1, stable[index + 1].1);
            spline_segment(at, index, pressures, &segment, size, &mut self.spline);
            self.along += segment.gap;
            self.segments += 1;
        }
        let before = settled.checked_sub(1).map(|at| self.spline[at]);
        measure(before, &mut self.spline[settled..]);
        if settled < 2 && self.spline.len() >= 2 {
            self.spline[0].toward = self.spline[1].toward;
        }
        let mut tail = Vec::new();
        let mut along = self.along;
        for index in self.segments..count.saturating_sub(1) {
            let segment = Segment::of(point(index).0, point(index + 1).0, along, total, size);
            let pressures = (point(index).1, point(index + 1).1);
            spline_segment(at, index, pressures, &segment, size, &mut tail);
            along += segment.gap;
        }
        measure(self.spline.last().copied(), &mut tail);
        let spline = &self.spline;
        let joined = spline.len() + tail.len();
        let spline_point = |at: usize| match (at, spline.len()) {
            (0, 1) => StrokePoint {
                toward: tail.first().map_or(Vec2::ZERO, |next| next.toward),
                ..spline[0]
            },
            (at, settled) if at < settled => spline[at],
            (at, settled) => tail[at - settled],
        };
        let first = spline_point(0);
        let end = spline_point(joined - 1);
        let run = end.run;
        if run < size || joined == 1 {
            return vec![fill(
                smooth_closed(&dot_of(first, size, THINNING)),
                Vec::new(),
            )];
        }
        if self.spline.len() >= 2 {
            let edges = self
                .edges
                .get_or_insert_with(|| Edges::new(self.spline[0], 0.0));
            while self.visited + 1 < self.spline.len()
                && run - self.spline[self.visited].run >= size * END_NOISE
            {
                let sides = Sides {
                    left: &mut self.left,
                    right: &mut self.right,
                };
                let index = self.visited;
                let next = Some(self.spline[index + 1]);
                edges.visit(
                    index,
                    self.spline[index],
                    next,
                    run,
                    size,
                    THINNING,
                    false,
                    sides,
                );
                self.visited += 1;
            }
        }
        let mut edges = self.edges.unwrap_or_else(|| Edges::new(first, 0.0));
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for index in self.visited..joined {
            let next = (index + 1 < joined).then(|| spline_point(index + 1));
            let sides = Sides {
                left: &mut left,
                right: &mut right,
            };
            edges.visit(
                index,
                spline_point(index),
                next,
                run,
                size,
                THINNING,
                false,
                sides,
            );
        }
        if (self.left.is_empty() && left.is_empty()) || (self.right.is_empty() && right.is_empty())
        {
            return vec![fill(
                smooth_closed(&dot_of(first, size, THINNING)),
                Vec::new(),
            )];
        }
        let cap = end_cap(end, edges.radius, false);
        self.settle_pieces(&fill);
        let (from_left, from_right) = self
            .boundary
            .map_or((0, 0), |(left, right)| (left + 1, right + 1));
        let left_side: Vec<Pos2> = self.left[from_left..]
            .iter()
            .chain(&left)
            .copied()
            .collect();
        let right_side: Vec<Pos2> = self.right[from_right..]
            .iter()
            .chain(&right)
            .copied()
            .collect();
        let vertices: Vec<Pos2> = left_side
            .iter()
            .copied()
            .chain(cap)
            .chain(right_side.iter().rev().copied())
            .collect();
        let left_first = self
            .left
            .first()
            .or(left.first())
            .copied()
            .expect("a left side");
        let right_first = self
            .right
            .first()
            .or(right.first())
            .copied()
            .expect("a right side");
        let (start, closing, seams) = match self.boundary {
            Some((bl, br)) => (
                middle(self.left[bl], self.left[bl + 1]),
                self.right[br],
                vec![self.seam(bl, br)],
            ),
            None => (middle(right_first, left_first), left_first, Vec::new()),
        };
        let mut path = Path::new().move_to(start);
        for (index, vertex) in vertices.iter().enumerate() {
            let next = vertices.get(index + 1).copied().unwrap_or(closing);
            path = path.quad_to(*vertex, middle(*vertex, next));
        }
        let mut drawn = self.pieces.clone();
        drawn.push(fill(path.close(), seams));
        drawn
    }

    fn fold(&mut self) {
        let size = self.size;
        let streaming = self.streaming.get_or_insert_with(|| {
            self.folded = 1;
            Streaming::new(self.raw[0])
        });
        let end = self.raw.len() - 1;
        for index in self.folded..end {
            let before = streaming.out[streaming.out.len() - 1].0;
            if streaming.take(self.raw[index], size, false, false) {
                let at = streaming.out[streaming.out.len() - 1].0;
                self.length += (at - before).length();
            }
        }
        self.folded = self.folded.max(end);
        if self.spline.is_empty() {
            self.spline.push(stroke_point(streaming.out[0]));
        }
    }

    fn seam(&self, left: usize, right: usize) -> [Pos2; 2] {
        [
            middle(self.left[left], self.left[left + 1]),
            middle(self.right[right + 1], self.right[right]),
        ]
    }

    fn settle_pieces(&mut self, fill: &impl Fn(Path, Vec<[Pos2; 2]>) -> Rc<PathPaint>) {
        loop {
            let (from_left, from_right) = self
                .boundary
                .map_or((0, 0), |(left, right)| (left + 1, right + 1));
            if self.left.len() < from_left + PIECE + 1 || self.right.len() < from_right + PIECE + 1
            {
                return;
            }
            let (bl, br) = (self.left.len() - 2, self.right.len() - 2);
            let start_point = middle(self.right[0], self.left[0]);
            let (start, mut seams) = match self.boundary {
                Some((left, right)) => (
                    middle(self.left[left], self.left[left + 1]),
                    vec![self.seam(left, right)],
                ),
                None => (start_point, Vec::new()),
            };
            seams.push(self.seam(bl, br));
            let mut path = Path::new().move_to(start);
            for index in from_left..=bl {
                path = path.quad_to(
                    self.left[index],
                    middle(self.left[index], self.left[index + 1]),
                );
            }
            path = path.line_to(middle(self.right[br + 1], self.right[br]));
            for index in (from_right..=br).rev() {
                let next = match index {
                    0 => start_point,
                    index => middle(self.right[index], self.right[index - 1]),
                };
                path = path.quad_to(self.right[index], next);
            }
            self.pieces.push(fill(path.close(), seams));
            self.boundary = Some((bl, br));
        }
    }
}
