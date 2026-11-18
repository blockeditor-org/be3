use super::*;
use crate::color::Hsva;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Heard {
    Dragging(ValueDrag),
    Value(Color32),
}

#[test]
fn dragging_a_color_picker_writes_each_move_between_its_drag_changes() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::new());
    let picker = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker
                    @node_ref=&picker
                    value={Binding::new(Hsva::new(0.0, 1.0, 1.0, 1.0).to_color(), move |next| {
                        heard.update(|heard| heard.push(Heard::Value(next)))
                    })}
                    on_drag_change={move |change| {
                        heard.update(|heard| heard.push(Heard::Dragging(change)))
                    }}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());

    harness.drag(
        pos2(area.left() + 4.0, area.top() + 4.0),
        pos2(area.left() + PICKER_WIDTH / 2.0, area.top() + 78.0),
    );
    harness.frame(Vec::new());

    let heard = heard.get_untracked();
    assert_eq!(
        heard.first(),
        Some(&Heard::Dragging(ValueDrag::Started)),
        "the drag starts before anything is written, so an owner can hold what follows"
    );
    assert_eq!(heard.last(), Some(&Heard::Dragging(ValueDrag::Kept)));
    let values: Vec<Color32> = heard
        .iter()
        .filter_map(|heard| match heard {
            Heard::Value(color) => Some(*color),
            Heard::Dragging(_) => None,
        })
        .collect();
    assert!(values.len() >= 2, "each move writes the value: {heard:?}");
    let reported = Hsva::from_color(*values.last().expect("the drag wrote"));
    assert!((reported.saturation - 0.5).abs() < 0.02);
    assert!((reported.value - 0.5).abs() < 0.02);
    assert_eq!(
        heard
            .iter()
            .filter(|heard| matches!(heard, Heard::Dragging(_)))
            .count(),
        2,
        "one drag starts and ends once"
    );
}
