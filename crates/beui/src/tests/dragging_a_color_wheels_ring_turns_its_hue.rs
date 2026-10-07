use super::*;
use crate::color::Hsva;
use crate::reactive::view;
use crate::styled::{ColorWheel, WHEEL_SIZE};

#[test]
fn dragging_a_color_wheels_ring_turns_its_hue() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let wheel = NodeRef::new();
    let named = wheel.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <ColorWheel
                    @node_ref=&named
                    value={Hsva::new(0.0, 1.0, 1.0, 1.0).to_color()}
                    on_change={move |color| changed.borrow_mut().push(color)}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let rect = harness.rect(wheel.get());
    let centre = pos2(
        rect.left() + WHEEL_SIZE / 2.0,
        rect.top() + WHEEL_SIZE / 2.0,
    );
    let along_ring = WHEEL_SIZE / 2.0 - 11.0;

    harness.drag(
        pos2(centre.x + along_ring, centre.y),
        pos2(centre.x, centre.y - along_ring),
    );
    harness.frame(Vec::new());

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1, "a drag reports once, when it ends");
    let turned = Hsva::from_color(changes[0]);
    assert!((turned.hue - 90.0).abs() < 1.0, "{turned:?}");
    assert_eq!((turned.saturation, turned.value), (1.0, 1.0));
}
