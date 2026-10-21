use std::f32::consts::PI;

use block_editor_beui::beui::{Pos2, Vec2, pos2};

const PRESSURE_RATE: f32 = 0.275;
const HALF_TURN: f32 = PI + 0.0001;
const SIMULATED_PRESSURE: f32 = 0.5;
const PRESSED_THINNING: f32 = 0.75;
const SIMULATED_THINNING: f32 = 0.5;
const SMOOTHING: f32 = 0.5;
const STREAMLINE: f32 = 0.5;
const TAPER: f32 = 3.0;
const PACE: f32 = 16.0;
const DOT_STEPS: usize = 16;

#[derive(Clone, Copy)]
struct StrokePoint {
    at: Pos2,
    pressure: f32,
    toward: Vec2,
    distance: f32,
    run: f32,
}

pub(crate) fn outline(points: &[Pos2], pressures: &[f32], size: f32, last: bool) -> Vec<Pos2> {
    let simulate = pressures.is_empty();
    let pressure_at = |at: usize| {
        pressures
            .get(at)
            .or(pressures.last())
            .copied()
            .unwrap_or(SIMULATED_PRESSURE)
            .clamp(0.0, 1.0)
    };
    let raw: Vec<(Pos2, f32)> = points
        .iter()
        .enumerate()
        .map(|(at, point)| (*point, pressure_at(at)))
        .collect();
    let points = stroke_points(raw, size, last);
    outline_of(&points, size, simulate, last)
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

fn stroke_points(mut raw: Vec<(Pos2, f32)>, size: f32, last: bool) -> Vec<StrokePoint> {
    let Some(&(first, first_pressure)) = raw.first() else {
        return Vec::new();
    };
    let follow = 0.15 + (1.0 - STREAMLINE) * 0.85;
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
    if raw.len() == 1 {
        raw.push((first + Vec2::splat(1.0), first_pressure));
    }
    let mut out = vec![StrokePoint {
        at: first,
        pressure: first_pressure,
        toward: Vec2::splat(1.0),
        distance: 0.0,
        run: 0.0,
    }];
    let mut long_enough = false;
    let mut run = 0.0;
    let end = raw.len() - 1;
    for (index, (point, pressure)) in raw.into_iter().enumerate().skip(1) {
        let before = out[out.len() - 1];
        let at = match last && index == end {
            true => point,
            false => before.at + (point - before.at) * follow,
        };
        if at == before.at {
            continue;
        }
        let distance = (at - before.at).length();
        run += distance;
        if index < end && !long_enough {
            if run < size {
                continue;
            }
            long_enough = true;
        }
        out.push(StrokePoint {
            at,
            pressure,
            toward: unit(before.at - at),
            distance,
            run,
        });
    }
    out[0].toward = out.get(1).map_or(Vec2::ZERO, |next| next.toward);
    out
}

fn simulated(before: f32, distance: f32, size: f32) -> f32 {
    let speed = (distance / (size * 4.0).max(PACE)).min(1.0);
    let rest = (1.0 - speed).min(1.0);
    (before + (rest - before) * (speed * PRESSURE_RATE)).min(1.0)
}

fn outline_of(points: &[StrokePoint], size: f32, simulate: bool, last: bool) -> Vec<Pos2> {
    let (Some(first), Some(end)) = (points.first(), points.last()) else {
        return Vec::new();
    };
    if size <= 0.0 || size.is_nan() {
        return Vec::new();
    }
    let thinning = match simulate {
        true => SIMULATED_THINNING,
        false => PRESSED_THINNING,
    };
    let radius_of = |pressure: f32| size * (0.5 - thinning * (0.5 - pressure));
    let total = end.run;
    if total < size {
        return disc(first.at, radius_of(first.pressure).max(size * 0.25));
    }
    let taper = size * TAPER;
    let least = (size * SMOOTHING).powi(2);
    let mut left: Vec<Pos2> = Vec::new();
    let mut right: Vec<Pos2> = Vec::new();
    let mut before_pressure = points.iter().take(10).fold(first.pressure, |acc, point| {
        let pressure = match simulate {
            true => simulated(acc, point.distance, size),
            false => point.pressure,
        };
        (acc + pressure) / 2.0
    });
    let mut radius = radius_of(end.pressure);
    let mut first_radius = None;
    let mut before_toward = first.toward;
    let (mut held_left, mut held_right) = (first.at, first.at);
    let mut sharp_before = false;
    for (index, point) in points.iter().enumerate() {
        let is_last = index == points.len() - 1;
        if !is_last && total - point.run < 3.0 {
            continue;
        }
        let pressure = match simulate {
            true => simulated(before_pressure, point.distance, size),
            false => point.pressure,
        };
        radius = radius_of(pressure);
        first_radius.get_or_insert(radius);
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
        radius = (radius * starting.min(ending)).max(0.01);
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
        before_pressure = pressure;
        before_toward = point.toward;
    }
    let start = first.at;
    let finish = match points.len() > 1 {
        true => end.at,
        false => first.at + Vec2::splat(1.0),
    };
    if points.len() == 1 || left.is_empty() || right.is_empty() {
        return disc(start, first_radius.unwrap_or(radius).max(size * 0.25));
    }
    let mut end_cap = Vec::new();
    match last {
        true => end_cap.push(finish),
        false => {
            let direction = across(-end.toward);
            let from = finish + direction * radius;
            for step in 1..29 {
                end_cap.push(around(from, finish, HALF_TURN * 3.0 * step as f32 / 29.0));
            }
        }
    }
    right.reverse();
    left.into_iter().chain(end_cap).chain(right).collect()
}
