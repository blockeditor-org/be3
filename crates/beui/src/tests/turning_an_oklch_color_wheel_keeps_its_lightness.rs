use super::*;
use crate::color::Oklch;
use crate::reactive::view;
use crate::styled::{OklchColorWheel, WHEEL_SIZE};

#[test]
fn turning_an_oklch_color_wheel_keeps_its_lightness() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let wheel = NodeRef::new();
    let named = wheel.clone();
    let start = Oklch::new(0.6, 0.06, 0.0, 1.0);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <OklchColorWheel
                    @node_ref=&named
                    value={start.to_color()}
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
        pos2(centre.x - along_ring, centre.y),
    );
    harness.frame(Vec::new());

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let turned = Oklch::from_color(changes[0]);
    assert!((turned.hue - 180.0).abs() < 2.0, "{turned:?}");
    assert!((turned.lightness - 0.6).abs() < 0.01, "{turned:?}");
    assert!((turned.chroma - 0.06).abs() < 0.01, "{turned:?}");
}
