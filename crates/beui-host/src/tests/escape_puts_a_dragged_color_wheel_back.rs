use super::*;
use crate::color::Hsva;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_puts_a_dragged_color_wheel_back() {
    let _root = crate::reactive::RootScope::new();
    let start = Hsva::new(0.0, 1.0, 1.0, 1.0).to_color();
    let value = create_signal(start);
    let drags = create_signal(Vec::new());
    let wheel = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorWheel
                    @node_ref=&wheel
                    value={value}
                    on_drag_change={move |change| drags.update(|drags| drags.push(change))}
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

    harness.down(pos2(centre.x + along_ring, centre.y), PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(pos2(centre.x, centre.y - along_ring))]);
    harness.frame(Vec::new());
    let turned = Hsva::from_color(value.get_untracked());
    assert!((turned.hue - 90.0).abs() < 1.0, "the ring turns as it is dragged: {turned:?}");

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.up(pos2(centre.x, centre.y - along_ring), PointerButton::Primary);
    harness.frame(Vec::new());

    assert_eq!(value.get_untracked(), start, "Escape turns it back");
    assert_eq!(
        drags.get_untracked(),
        vec![ValueDrag::Started, ValueDrag::Cancelled]
    );
}
