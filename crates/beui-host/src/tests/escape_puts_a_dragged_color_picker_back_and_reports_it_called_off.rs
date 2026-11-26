use super::*;
use crate::color::Hsva;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_puts_a_dragged_color_picker_back_and_reports_it_called_off() {
    let _root = crate::reactive::RootScope::new();
    let start = Hsva::new(0.0, 1.0, 1.0, 1.0).to_color();
    let value = create_signal(start);
    let drags = create_signal(Vec::new());
    let picker = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker
                    @node_ref=&picker
                    value={value}
                    on_drag_change={move |change| drags.update(|drags| drags.push(change))}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());
    let pressed = pos2(area.left() + 4.0, area.top() + 4.0);
    let middle = pos2(area.left() + PICKER_WIDTH / 2.0, area.top() + 78.0);

    harness.down(pressed, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(middle)]);
    harness.frame(Vec::new());
    let moved = Hsva::from_color(value.get_untracked());
    assert!(
        (moved.saturation - 0.5).abs() < 0.02 && (moved.value - 0.5).abs() < 0.02,
        "the area follows the pointer: {moved:?}"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.up(middle, PointerButton::Primary);
    harness.frame(Vec::new());

    assert_eq!(
        value.get_untracked(),
        start,
        "Escape writes back the colour the drag started from"
    );
    assert_eq!(
        drags.get_untracked(),
        vec![ValueDrag::Started, ValueDrag::Cancelled]
    );
}
