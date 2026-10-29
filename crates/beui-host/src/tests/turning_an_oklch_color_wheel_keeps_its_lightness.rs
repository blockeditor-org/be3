use super::*;
use crate::color::Oklch;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn turning_an_oklch_color_wheel_keeps_its_lightness() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let wheel = NodeRef::new();
    let start = Oklch::new(0.6, 0.06, 0.0, 1.0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <OklchColorWheel
                    @node_ref=&wheel
                    value={Binding::new(start.to_color(), move |next| changes.update(|changes| changes.push(next)))}
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

    let changes = changes.get_untracked();
    assert_eq!(changes.len(), 1);
    let turned = Oklch::from_color(changes[0]);
    assert!((turned.hue - 180.0).abs() < 2.0, "{turned:?}");
    assert!((turned.lightness - 0.6).abs() < 0.01, "{turned:?}");
    assert!((turned.chroma - 0.06).abs() < 0.01, "{turned:?}");
}
