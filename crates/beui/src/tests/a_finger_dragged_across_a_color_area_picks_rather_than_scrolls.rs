use super::*;
use crate::color::Hsva;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_finger_dragged_across_a_color_area_picks_rather_than_scrolls() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let picker = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Fixed(200.0)>
                    <ColorPicker
                        @node_ref=&picker
                        value={Binding::new(Hsva::new(120.0, 1.0, 1.0, 1.0).to_color(), move |next| changes.update(|changes| changes.push(next)))}
                    />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());
    let start = pos2(area.left() + 10.0, area.top() + 10.0);
    let end = pos2(area.left() + PICKER_WIDTH / 4.0, area.top() + 117.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, pos2(start.x + 20.0, start.y + 40.0));
    harness.touch(TouchPhase::Move, end);
    harness.touch(TouchPhase::End, end);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(picker.get()).top(),
        area.top(),
        "the scroll stayed put"
    );
    let changes = changes.get_untracked();
    assert_eq!(changes.len(), 1);
    let picked = Hsva::from_color(changes[0]);
    assert!((picked.hue - 120.0).abs() < 1.0);
    assert!((picked.saturation - 0.25).abs() < 0.02);
    assert!((picked.value - 0.25).abs() < 0.02);
}
