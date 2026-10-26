use std::rc::Rc;

use super::*;
use crate::primitives::*;
use crate::reactive::Draw;
use crate::reactive::*;

#[test]
fn a_drawing_given_less_room_than_its_size_keeps_to_that_room() {
    let _root = crate::reactive::RootScope::new();
    let given = create_signal(Rect::ZERO);
    let draw: Draw = Rc::new(move |painter: &Painter, rect: Rect| {
        given.set(rect);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(200.0, 80.0)),
            0.0,
            Color32::WHITE,
        );
    });
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame width=100.0>
                    <Drawing draw={draw} size={Some(Vec2::new(200.0, 80.0))} />
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let given = given.get_untracked();
    assert_eq!(
        given.size(),
        Vec2::new(100.0, 40.0),
        "the callback is given the room layout left, at the drawing's aspect"
    );
    let clips: Vec<Rect> = output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Rect { clip, color, .. } if *color == Color32::WHITE => {
                Some(*clip)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        clips,
        vec![given],
        "what it draws past that room is cut off rather than spilling over its neighbours"
    );
}
