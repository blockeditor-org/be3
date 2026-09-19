use super::*;
use crate::reactive::{Direction, ItemSize, List, build, view, with_document};

const SCALE: f32 = 1.5;

#[test]
fn percent_children_land_on_whole_device_pixels() {
    let children = Rc::new(RefCell::new(Vec::new()));
    let sink = children.clone();

    let document = build(move || {
        let tree = view! {
            <List direction=Direction::Horizontal spacing=1.0>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0></List>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0></List>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0></List>
            </List>
        };
        sink.replace(with_document(|document| document.children(tree)));
        tree
    });

    let mut harness = Harness::sized(document, Vec2::new(301.0, 200.0));
    harness.context.set_pixels_per_point(SCALE);
    harness.frame(Vec::new());

    let edges: Vec<(f32, f32)> = children
        .borrow()
        .iter()
        .map(|child| harness.rect(*child))
        .map(|rect| (rect.min.x * SCALE, rect.max.x * SCALE))
        .collect();

    for (left, right) in &edges {
        assert_eq!(*left, left.round(), "a left edge missed the pixel grid");
        assert_eq!(*right, right.round(), "a right edge missed the pixel grid");
    }

    assert_eq!(edges[0].0, 0.0);
    assert_eq!(edges[2].1, 452.0);
    assert_eq!((edges[1].0 - edges[0].1).round(), 2.0);
    assert_eq!((edges[2].0 - edges[1].1).round(), 2.0);

    let widths: Vec<f32> = edges.iter().map(|(left, right)| right - left).collect();
    assert_eq!(widths, vec![149.0, 150.0, 149.0]);
}
