use super::*;
use crate::reactive::{NodeRef, with_reactive_scope};
use crate::styled::ModalSheet;

#[test]
fn a_modal_sheet_slides_in_and_out() {
    let content = NodeRef::new();
    let (open, set_open) = create_signal(false);
    let document = build({
        let (content, set_open) = (content.clone(), set_open.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <ModalSheet open={open} fit=true on_close={move || set_open.set(false)}>
                        <Frame @node_ref=&content height=120.0 />
                    </ModalSheet>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    let top = |harness: &Harness| {
        harness
            .document()
            .node_rect(content.get())
            .map(|rect| rect.top())
    };
    let opening = set_open.clone();
    with_reactive_scope(harness.document_mut(), move || opening.set(true));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let entering = top(&harness).expect("the sheet is laid out as it enters");
    assert!(
        entering > 480.0,
        "an opened sheet starts below where it rests and slides up, and is at {entering}"
    );
    harness.settle();
    assert_eq!(top(&harness), Some(480.0), "then rests on the bottom edge");

    let outside = harness.find("sheet.outside");
    let above = harness.center(outside);
    harness.click(above);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let leaving = top(&harness).expect("a closed sheet stays on screen while it leaves");
    assert!(
        leaving > 480.0 && leaving < 600.0,
        "a tap above the sheet slides it down rather than removing it, to {leaving}"
    );
    harness.settle();
    assert_eq!(top(&harness), None, "and it is gone once it has left");
}
