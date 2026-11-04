use std::rc::Rc;

use super::*;
use crate::primitives::*;
use crate::reactive::Draw;
use crate::reactive::*;

#[test]
fn a_drawing_paints_what_its_callback_puts_in_the_rectangle_it_is_given() {
    let _root = crate::reactive::RootScope::new();
    let painted = create_signal(0);
    let drawn = move |color: Color32| -> Draw {
        Rc::new(move |painter: &Painter, rect: Rect| {
            if !core_painter(painter).ctx().is_verifying_paint() {
                painted.update(|painted| *painted += 1);
            }
            painter.rect_filled(rect, 0.0, color);
        })
    };
    let draw = create_signal(drawn(Color32::WHITE));
    let document = build_node(move || {
        view! {
            <Drawing draw={draw} />
        }
    });
    let mut harness = Harness::new(document);

    let output = harness.frame(Vec::new());
    assert_eq!(
        painted.get_untracked(),
        1,
        "the callback paints the frame it is in"
    );
    assert_eq!(fills(&output), vec![(VIEWPORT, Color32::WHITE)]);

    harness.frame(Vec::new());
    assert_eq!(
        painted.get_untracked(),
        1,
        "an unchanged frame reuses what it painted"
    );

    let black = drawn(Color32::BLACK);
    with_installed(harness.document_mut(), |_| {
        draw.set_unconditionally(black);
    });
    let output = harness.frame(Vec::new());
    assert_eq!(painted.get_untracked(), 2, "a new callback repaints");
    assert_eq!(fills(&output), vec![(VIEWPORT, Color32::BLACK)]);
}

fn fills(output: &crate::native::FrameOutput) -> Vec<(Vec2, Color32)> {
    output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Rect { rect, color, .. } => Some((rect.size(), *color)),
            _ => None,
        })
        .collect()
}
