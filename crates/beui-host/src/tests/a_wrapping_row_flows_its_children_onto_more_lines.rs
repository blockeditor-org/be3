use super::*;
use crate::native::with_document;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_wrapping_row_flows_its_children_onto_more_lines() {
    let _root = crate::reactive::RootScope::new();
    let tiles = create_signal(Vec::new());

    let document = build_node(move || {
        let row = view! {
            <List direction=Direction::Horizontal wrap=true spacing=10.0>
                <Frame @sizing=ItemSize::Fixed(40.0) height=20.0 />
                <Frame @sizing=ItemSize::Fixed(40.0) height=20.0 />
                <Frame @sizing=ItemSize::Fixed(40.0) height=20.0 />
            </List>
        };
        tiles.set(with_document(|document| document.children(row)));
        row
    });

    let mut harness = Harness::sized(document, Vec2::new(100.0, 200.0));
    harness.frame(Vec::new());

    let tiles = tiles.get_untracked();
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
