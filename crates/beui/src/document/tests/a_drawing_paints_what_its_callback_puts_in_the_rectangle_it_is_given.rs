use std::rc::Rc;

use super::*;
use crate::reactive::{Draw, Drawing, create_signal, view};

#[test]
fn a_drawing_paints_what_its_callback_puts_in_the_rectangle_it_is_given() {
    let painted = Rc::new(Cell::new(0));
    let drawn = {
        let painted = painted.clone();
        move |color: Color32| -> Draw {
            let painted = painted.clone();
            Rc::new(move |painter: &Painter, rect: Rect| {
                painted.set(painted.get() + 1);
                painter.rect_filled(rect, 0.0, color);
            })
        }
    };
    let (draw, set_draw) = create_signal(drawn(Color32::WHITE));
    let document = build(move || {
        view! {
            <Drawing draw={draw} />
        }
    });
    let mut harness = Harness::new(document);

    let output = harness.frame(Vec::new());
    assert_eq!(painted.get(), 1, "the callback paints the frame it is in");
    assert_eq!(fills(&output), vec![(VIEWPORT, Color32::WHITE)]);

    harness.frame(Vec::new());
    assert_eq!(
        painted.get(),
        1,
        "an unchanged frame reuses what it painted"
    );

    let black = drawn(Color32::BLACK);
    with_installed(harness.document_mut(), |_| {
        set_draw.set_unconditionally(black);
    });
    let output = harness.frame(Vec::new());
    assert_eq!(painted.get(), 2, "a new callback repaints");
    assert_eq!(fills(&output), vec![(VIEWPORT, Color32::BLACK)]);
}

fn fills(output: &crate::FrameOutput) -> Vec<(Vec2, Color32)> {
    output
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Rect { rect, color, .. } => Some((rect.size(), *color)),
            _ => None,
        })
        .collect()
}
