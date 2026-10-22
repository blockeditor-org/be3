use std::f32::consts::PI;
use std::time::Instant;

use block_editor_beui::beui::{Path, Pos2, Vec2, pos2};

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
    let pressure_at = |at: usize| {
        pressures
            .get(at)
            .or(pressures.last())
            .copied()
            .unwrap_or(RESTING_PRESSURE)
            .clamp(0.0, 1.0)
    };
    let raw: Vec<(Pos2, f32)> = points
        .iter()
        .enumerate()
        .map(|(at, point)| (*point, pressure_at(at)))
        .collect();
    let points = splined(&streamlined(raw, size, last), size);
    smooth_closed(&outline(&points, size, THINNING, last))
}

fn smooth_closed(outline: &[Pos2]) -> Path {
    let (Some(&first), Some(&end)) = (outline.first(), outline.last()) else {
        return Path::new();
    };
    if outline.len() < 3 {
        return Path::new();
    }
    let middle = |a: Pos2, b: Pos2| a + (b - a) * 0.5;
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

fn streamlined(mut raw: Vec<(Pos2, f32)>, size: f32, last: bool) -> Vec<StrokePoint> {
    let Some(&(first, first_pressure)) = raw.first() else {
        return Vec::new();
    };
    let lag = 1.0 - (0.15 + (1.0 - STREAMLINE) * 0.85);
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
    let mut out = vec![(first, first_pressure)];
    let mut long_enough = false;
    let mut run = 0.0;
    let end = raw.len() - 1;
    for (index, (point, pressure)) in raw.into_iter().enumerate().skip(1) {
        let before = out[out.len() - 1].0;
        let at = match last && index == end {
            true => point,
            false => before + (point - before) * (1.0 - lag.powf((point - before).length() / size)),
        };
        if at == before {
            continue;
        }
        let distance = (at - before).length();
        run += distance;
        if index < end && !long_enough {
            if run < size {
                continue;
            }
            long_enough = true;
        }
        out.push((at, pressure));
    }
    out.into_iter()
        .map(|(at, pressure)| StrokePoint {
            at,
            pressure,
            toward: Vec2::ZERO,
            run: 0.0,
        })
        .collect()
}

fn splined(points: &[StrokePoint], size: f32) -> Vec<StrokePoint> {
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    let last = points.len() as isize - 1;
    let at = |index: isize| -> Pos2 {
        match index {
            -1 if last > 0 => points[0].at + (points[0].at - points[1].at),
            index if index > last && last > 0 => {
                let end = points[last as usize].at;
                end + (end - points[last as usize - 1].at)
            }
            index => points[index.clamp(0, last) as usize].at,
        }
    };
    let knot = |a: Pos2, b: Pos2| (b - a).length().sqrt().max(f32::EPSILON);
    let total: f32 = points
        .windows(2)
        .map(|pair| (pair[1].at - pair[0].at).length())
        .sum();
    let taper = size * TAPER;
    let mut along = 0.0;
    let mut out = vec![first];
    for (index, pair) in points.windows(2).enumerate() {
        let index = index as isize;
        let (p0, p1, p2, p3) = (at(index - 1), at(index), at(index + 1), at(index + 2));
        let (d0, d1, d2) = (knot(p0, p1), knot(p1, p2), knot(p2, p3));
        let leave = ((p1 - p0) / d0 - (p2 - p0) / (d0 + d1) + (p2 - p1) / d1) * d1;
        let arrive = ((p2 - p1) / d1 - (p3 - p1) / (d1 + d2) + (p3 - p2) / d2) * d1;
        let (c1, c2) = (p1 + leave / 3.0, p2 - arrive / 3.0);
        let chord = unit(p2 - p1);
        let bend = cross(c1 - p1, chord).abs().max(cross(c2 - p1, chord).abs());
        let gap = (p2 - p1).length();
        let tapered = along < taper || along + gap > total - taper;
        along += gap;
        let stride = match tapered {
            true => gap / (size * TAPER_STRIDE),
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
                pressure: pair[0].pressure + (pair[1].pressure - pair[0].pressure) * t,
                toward: Vec2::ZERO,
                run: 0.0,
            });
        }
    }
    let mut run = 0.0;
    for index in 1..out.len() {
        let step = out[index - 1].at - out[index].at;
        run += step.length();
        out[index].toward = unit(step);
        out[index].run = run;
    }
    out[0].toward = out.get(1).map_or(Vec2::ZERO, |next| next.toward);
    out
}

fn outline(points: &[StrokePoint], size: f32, thinning: f32, last: bool) -> Vec<Pos2> {
    let (Some(first), Some(end)) = (points.first(), points.last()) else {
        return Vec::new();
    };
    let radius_of = |pressure: f32| size * (0.5 - thinning * (0.5 - pressure));
    let total = end.run;
    if total < size || points.len() == 1 {
        return disc(first.at, radius_of(first.pressure).max(size * 0.25));
    }
    let taper = size * TAPER;
    let least = (size * SMOOTHING).powi(2);
    let mut left: Vec<Pos2> = Vec::new();
    let mut right: Vec<Pos2> = Vec::new();
    let mut radius = radius_of(end.pressure);
    let mut before_toward = first.toward;
    let (mut held_left, mut held_right) = (first.at, first.at);
    let mut sharp_before = false;
    for (index, point) in points.iter().enumerate() {
        let is_last = index == points.len() - 1;
        if !is_last && total - point.run < size * END_NOISE {
            continue;
        }
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
        radius = (radius_of(point.pressure) * starting.min(ending)).max(size * 0.005);
        let next_toward = points
            .get(index + 1)
            .map_or(point.toward, |next| next.toward);
        let next_turn = match is_last {
            true => 1.0,
            false => dot(point.toward, next_toward),
        };
        let turn = dot(point.toward, before_toward);
        let sharp = turn < 0.0 && !sharp_before;
        let sharp_next = next_turn < 0.0;
        if sharp || sharp_next {
            let offset = across(before_toward) * radius;
            for step in 0..=13 {
                let t = step as f32 / 13.0;
                held_left = around(point.at - offset, point.at, HALF_TURN * t);
                left.push(held_left);
                held_right = around(point.at + offset, point.at, -HALF_TURN * t);
                right.push(held_right);
            }
            sharp_before = sharp_next;
            continue;
        }
        sharp_before = false;
        if is_last {
            let offset = across(point.toward) * radius;
            left.push(point.at - offset);
            right.push(point.at + offset);
            continue;
        }
        let offset = across(lerp(next_toward, point.toward, next_turn)) * radius;
        let at_left = point.at - offset;
        if index <= 1 || dot(held_left - at_left, held_left - at_left) > least {
            left.push(at_left);
            held_left = at_left;
        }
        let at_right = point.at + offset;
        if index <= 1 || dot(held_right - at_right, held_right - at_right) > least {
            right.push(at_right);
            held_right = at_right;
        }
        before_toward = point.toward;
    }
    if left.is_empty() || right.is_empty() {
        return disc(first.at, radius_of(first.pressure).max(size * 0.25));
    }
    let mut end_cap = Vec::new();
    match last {
        true => end_cap.push(end.at),
        false => {
            let from = end.at + across(-end.toward) * radius;
            for step in 1..29 {
                end_cap.push(around(from, end.at, HALF_TURN * 3.0 * step as f32 / 29.0));
            }
        }
    }
    right.reverse();
    left.into_iter().chain(end_cap).chain(right).collect()
}
