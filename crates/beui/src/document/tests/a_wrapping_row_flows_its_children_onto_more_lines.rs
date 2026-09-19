use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, build, view, with_document};

#[test]
fn a_wrapping_row_flows_its_children_onto_more_lines() {
    let tiles = Rc::new(RefCell::new(Vec::new()));
    let sink = tiles.clone();

    let document = build(move || {
        let row = view! {
            <List direction=Direction::Horizontal wrap=true spacing=10.0>
                <Frame @sizing=ItemSize::Fixed(40.0) height=20.0 />
                <Frame @sizing=ItemSize::Fixed(40.0) height=20.0 />
                <Frame @sizing=ItemSize::Fixed(40.0) height=20.0 />
            </List>
        };
        sink.replace(with_document(|document| document.children(row)));
        row
    });

    let mut harness = Harness::sized(document, Vec2::new(100.0, 200.0));
    harness.frame(Vec::new());

    let tiles = tiles.borrow();
    assert_eq!(
        harness.rect(tiles[0]),
        Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 20.0))
    );
    assert_eq!(
        harness.rect(tiles[1]),
        Rect::from_min_size(pos2(50.0, 0.0), Vec2::new(40.0, 20.0))
    );
    assert_eq!(
        harness.rect(tiles[2]),
        Rect::from_min_size(pos2(0.0, 30.0), Vec2::new(40.0, 20.0))
    );
}
