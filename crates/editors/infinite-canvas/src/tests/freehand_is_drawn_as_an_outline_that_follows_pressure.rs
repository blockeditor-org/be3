use super::*;
use crate::freehand::Ink;
use block_editor_beui::beui::{Pos2, pos2};
use std::time::{Duration, Instant};

fn stroke(
    id: u128,
    top: f32,
    pressures: impl Fn(f32) -> Option<f32>,
    spacing: fn(f32) -> f32,
) -> CanvasEntity {
    let along: Vec<f32> = (0..=40).map(|step| spacing(step as f32 / 40.0)).collect();
    let points: Vec<CanvasPoint> = along
        .iter()
        .map(|t| CanvasPoint::new(-220.0 + 440.0 * t, top + 30.0 * (t * 9.0).sin()))
        .collect();
    let mut entity = path(Uuid::from_u128(id), &points);
    entity.shape = CanvasShape::Freehand;
    entity.style.stroke_width = 6.0;
    entity.pressures = along.iter().filter_map(|t| pressures(*t)).collect();
    entity
}

#[test]
fn freehand_is_drawn_as_an_outline_that_follows_pressure() {
    let pressed = stroke(
        1,
        -120.0,
        |t| Some(0.1 + 0.9 * (t * std::f32::consts::PI).sin()),
        |t| t,
    );
    let mut sped_up = stroke(2, 0.0, |_| None, |t| t * t);
    let drawn: Vec<Pos2> = sped_up
        .world_points()
        .iter()
        .map(|point| pos2(point.at().x, point.at().y))
        .collect();
    let origin = Instant::now();
    let mut ink = Ink::new(drawn[0], origin);
    sped_up.pressures =
        std::iter::once(ink.pressure())
            .chain(drawn.iter().enumerate().skip(1).map(|(frame, at)| {
                ink.sample(*at, origin + Duration::from_millis(16 * frame as u64))
            }))
            .collect();
    let corners = [
        CanvasPoint::new(-200.0, 100.0),
        CanvasPoint::new(-100.0, 180.0),
        CanvasPoint::new(0.0, 100.0),
        CanvasPoint::new(100.0, 180.0),
        CanvasPoint::new(200.0, 100.0),
    ];
    let dense: Vec<CanvasPoint> = corners
        .windows(2)
        .flat_map(|pair| {
            (0..10).map(move |step| {
                let t = step as f32 / 10.0;
                CanvasPoint::new(
                    pair[0].x + (pair[1].x - pair[0].x) * t,
                    pair[0].y + (pair[1].y - pair[0].y) * t,
                )
            })
        })
        .chain(corners.last().copied())
        .collect();
    let mut zigzag = path(Uuid::from_u128(3), &dense);
    zigzag.shape = CanvasShape::Freehand;
    zigzag.style.stroke_width = 4.0;
    zigzag.pressures = vec![0.5; dense.len()];

    let mut editor = editor(&[pressed, sped_up, zigzag]);
    editor.run();

    editor.snapshot("freehand_is_drawn_as_an_outline_that_follows_pressure");
}
