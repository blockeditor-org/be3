use super::*;
use crate::color::Hsva;
use crate::reactive::view;
use crate::styled::{ColorPicker, PICKER_WIDTH, Scroll};

#[test]
fn a_finger_dragged_across_a_color_area_picks_rather_than_scrolls() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let picker = NodeRef::new();
    let named = picker.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Fixed(200.0)>
                    <ColorPicker
                        @node_ref=&named
                        value={Hsva::new(120.0, 1.0, 1.0, 1.0).to_color()}
                        on_change={move |color| changed.borrow_mut().push(color)}
                    />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());
    let start = pos2(area.left() + PICKER_WIDTH - 4.0, area.top() + 4.0);
    let end = pos2(area.left() + PICKER_WIDTH / 4.0, area.top() + 117.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, pos2(start.x - 20.0, start.y + 40.0));
    harness.touch(TouchPhase::Move, end);
    harness.touch(TouchPhase::End, end);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(picker.get()).top(),
        area.top(),
        "the scroll stayed put"
    );
    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let picked = Hsva::from_color(changes[0]);
    assert!((picked.hue - 120.0).abs() < 1.0);
    assert!((picked.saturation - 0.25).abs() < 0.02);
    assert!((picked.value - 0.25).abs() < 0.02);
}
