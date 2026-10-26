use super::*;
use crate::color::Hsva;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn dragging_a_color_picker_previews_the_color_and_reports_it_once() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let previews = create_signal(Vec::new());
    let picker = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker
                    @node_ref=&picker
                    value={Binding::new(Hsva::new(0.0, 1.0, 1.0, 1.0).to_color(), move |next| changes.update(|changes| changes.push(next)))}
                    on_preview={move |color| previews.update(|previews| previews.push(color))}
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

    let changes = changes.get_untracked();
    assert_eq!(changes.len(), 1, "a drag reports once, when it ends");
    let reported = Hsva::from_color(changes[0]);
    assert!((reported.saturation - 0.5).abs() < 0.02);
    assert!((reported.value - 0.5).abs() < 0.02);
    let previews = previews.get_untracked();
    assert!(previews.len() >= 2);
    assert_eq!(
        previews.last(),
        Some(&None),
        "the preview ends with the drag"
    );
}
