use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::styled::ModalSheet;

#[test]
fn a_modal_sheet_rests_where_it_would_when_measurement_comes_late() {
    let (prompt, late) = prompt_and_late(|| {
        let _root = crate::reactive::RootScope::new();
        let content = NodeRef::new();
        let open = create_signal(false);
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <ModalSheet open={open} fit=true on_close={move || open.set(false)}>
                        <Frame @node_ref=&content height=120.0 />
                    </ModalSheet>
                </List>
            }
        });
        let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
        harness.settle();
        open.set(true);
        harness.settle();
        let rested = harness.document().node_rect(content.get());
        let outside = harness.find("sheet.outside");
        harness.click(harness.center(outside));
        harness.settle();
        (rested, harness.document().node_rect(content.get()))
    });
    assert_eq!(prompt.0.map(|rect| rect.top()), Some(480.0));
    assert_eq!(prompt.1, None, "a tap outside closes it");
    assert_eq!(
        late, prompt,
        "late measurement settles where prompt measurement does"
    );
}
