use super::*;

use std::rc::Rc;

use beui_core::path::PathPaint;
use beui_types::{FillRule, Path, PathStyle};

#[test]
fn two_fills_meeting_on_a_seam_paint_as_one() {
    let ink = Color32::from_rgba_unmultiplied(20, 40, 200, 160);
    let seam = [pos2(30.0, 8.0), pos2(41.0, 56.0)];
    let halves = |seams: Vec<[Pos2; 2]>| {
        [
            Path::polygon([pos2(8.0, 8.0), seam[0], seam[1], pos2(8.0, 56.0)]),
            Path::polygon([seam[0], pos2(56.0, 8.0), pos2(56.0, 56.0), seam[1]]),
        ]
        .map(|half| {
            Rc::new(PathPaint::with_seams(
                half,
                PathStyle::Fill(FillRule::NonZero),
                seams.clone(),
            ))
        })
    };
    let whole = capture(Color32::WHITE, |painter| {
        painter.fill_path(
            &Path::polygon([
                pos2(8.0, 8.0),
                pos2(56.0, 8.0),
                pos2(56.0, 56.0),
                pos2(8.0, 56.0),
            ]),
            FillRule::NonZero,
            ink,
        );
    });
    let painted = |seams: Vec<[Pos2; 2]>| {
        capture(Color32::WHITE, |painter| {
            for half in halves(seams) {
                painter.paint_path(&half, ink);
            }
        })
    };
    let differing = |capture: &Capture| {
        (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let (held, wanted) = (capture.pixel(*x, *y), whole.pixel(*x, *y));
                held.iter()
                    .zip(wanted)
                    .any(|(held, wanted)| held.abs_diff(wanted) > 2)
            })
            .count()
    };

    assert!(
        differing(&painted(Vec::new())) > 0,
        "two fills blended along the edge they share show it"
    );
    assert_eq!(
        differing(&painted(vec![seam])),
        0,
        "two fills meeting on a seam paint as one fill"
    );
}
