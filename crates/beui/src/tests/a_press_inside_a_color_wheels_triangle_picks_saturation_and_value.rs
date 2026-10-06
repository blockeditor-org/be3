use super::*;
use crate::color::Hsva;
use crate::reactive::view;
use crate::styled::{ColorWheel, WHEEL_SIZE};

#[test]
fn a_press_inside_a_color_wheels_triangle_picks_saturation_and_value() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let wheel = NodeRef::new();
    let named = wheel.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <ColorWheel
                    @node_ref=&named
                    value={Hsva::new(200.0, 1.0, 1.0, 1.0).to_color()}
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

    harness.click(centre);
    harness.frame(Vec::new());

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let picked = Hsva::from_color(changes[0]);
    assert!((picked.hue - 200.0).abs() < 1.0, "{picked:?}");
    assert!((picked.saturation - 0.5).abs() < 0.02, "{picked:?}");
    assert!((picked.value - 2.0 / 3.0).abs() < 0.02, "{picked:?}");
}
