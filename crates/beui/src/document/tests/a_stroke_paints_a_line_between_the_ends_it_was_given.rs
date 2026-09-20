use super::*;
use crate::color::Color32;
use crate::reactive::{Frame, Stroke, build, view};

#[test]
fn a_stroke_paints_a_line_between_the_ends_it_was_given() {
    let document = build(move || {
        view! {
            <Frame width=40.0 height=40.0>
                <Stroke
                    from={pos2(4.0, 6.0)}
                    to={pos2(30.0, 22.0)}
                    width=3.0
                    color=Color32::WHITE
                />
            </Frame>
        }
    });

    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let drawn: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Line {
                from, to, width, ..
            } => Some((*from, *to, *width)),
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 1, "the stroke paints one line");
    assert_eq!(
        drawn[0],
        (pos2(4.0, 6.0), pos2(30.0, 22.0), 3.0),
        "the line runs between the ends, offset by where the node was placed"
    );
}
