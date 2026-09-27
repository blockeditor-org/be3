use super::*;
use crate::color::Hsva;
use crate::reactive::view;
use crate::styled::{ColorPicker, PICKER_WIDTH};

#[test]
fn dragging_a_color_picker_previews_the_color_and_reports_it_once() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let previews = Rc::new(RefCell::new(Vec::new()));
    let (changed, previewed) = (changes.clone(), previews.clone());
    let picker = NodeRef::new();
    let named = picker.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker
                    @node_ref=&named
                    value={Hsva::new(0.0, 1.0, 1.0, 1.0).to_color()}
                    on_change={move |color| changed.borrow_mut().push(color)}
                    on_preview={move |color| previewed.borrow_mut().push(color)}
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

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1, "a drag reports once, when it ends");
    let reported = Hsva::from_color(changes[0]);
    assert!((reported.saturation - 0.5).abs() < 0.02);
    assert!((reported.value - 0.5).abs() < 0.02);
    let previews = previews.borrow();
    assert!(previews.len() >= 2);
    assert_eq!(previews.last(), Some(&None), "the preview ends with the drag");
}
