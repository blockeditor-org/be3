use crate::freehand::{self, Ink};
use block_editor_beui::beui::{Pos2, pos2};
use std::time::{Duration, Instant};

const LENGTH_MS: f32 = 600.0;

fn at(ms: f32) -> Pos2 {
    let t = ms / LENGTH_MS;
    let eased = t * t * (3.0 - 2.0 * t);
    pos2(400.0 * eased, 60.0 * (eased * std::f32::consts::PI).sin())
}

fn sampled(hertz: f32, origin: Instant) -> (Vec<(f32, f32)>, Vec<Pos2>, Vec<f32>) {
    let mut ink = Ink::new(at(0.0), origin);
    let (mut curve, mut points, mut pressures) = (
        vec![(0.0, ink.pressure())],
        vec![at(0.0)],
        vec![ink.pressure()],
    );
    let count = (LENGTH_MS / 1000.0 * hertz).round() as usize;
    for sample in 1..=count {
        let ms = LENGTH_MS * sample as f32 / count as f32;
        let time = origin + Duration::from_secs_f32(ms / 1000.0);
        let pressure = ink.sample(at(ms), time);
        curve.push((ms, pressure));
        if points.last().is_none_or(|last| last.distance(at(ms)) > 1.0) {
            points.push(at(ms));
            pressures.push(pressure);
        }
    }
    (curve, points, pressures)
}

fn pressure_at(curve: &[(f32, f32)], ms: f32) -> f32 {
    let after = curve.iter().position(|(when, _)| *when >= ms).unwrap();
    if after == 0 {
        return curve[0].1;
    }
    let ((t0, p0), (t1, p1)) = (curve[after - 1], curve[after]);
    p0 + (p1 - p0) * (ms - t0) / (t1 - t0)
}

#[test]
fn simulated_pressure_is_the_same_at_any_sample_rate() {
    let origin = Instant::now();
    let rates = [30.0, 240.0, 1000.0].map(|hertz| sampled(hertz, origin));
    let (reference, ..) = &rates[2];
    let lightest = reference.iter().map(|(_, p)| *p).fold(1.0, f32::min);
    assert!(
        lightest < 0.4,
        "a fast stroke thins: the lightest pressure is {lightest}"
    );
    for (curve, ..) in &rates {
        for step in 1..=12 {
            let ms = LENGTH_MS * step as f32 / 12.0;
            let (got, want) = (pressure_at(curve, ms), pressure_at(reference, ms));
            assert!(
                (got - want).abs() < 0.05,
                "at {ms}ms one rate reads {got} and 1000 Hz reads {want}"
            );
        }
    }
    let outlines = rates
        .iter()
        .map(|(_, points, pressures)| freehand::stroke(points, pressures, 12.0, true).bounds())
        .collect::<Vec<_>>();
    for outline in &outlines {
        let reference = outlines[2];
        for (got, want) in [
            (outline.min.x, reference.min.x),
            (outline.min.y, reference.min.y),
            (outline.max.x, reference.max.x),
            (outline.max.y, reference.max.y),
        ] {
            assert!(
                (got - want).abs() < 2.0,
                "the outline spans {outline:?} where 1000 Hz spans {reference:?}"
            );
        }
    }
}
