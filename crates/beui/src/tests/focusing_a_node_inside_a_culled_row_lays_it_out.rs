use super::*;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Offset, view};

const ROWS: usize = 300;
const ROW_HEIGHT: f32 = 30.0;
const FOCUSED: usize = 250;

#[test]
fn focusing_a_node_inside_a_culled_row_lays_it_out() {
    let rows: Vec<NodeRef> = (0..ROWS).map(|_| NodeRef::new()).collect();
    let document = build({
        let rows = rows.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0)>
                        <List spacing=0.0>
                            <ForEach keys={indices(ROWS)}>
                                {move |index: usize| {
                                    let row = rows[index].clone();
                                    view! {
                                        <Frame @node_ref=&row>
                                            <Frame width=200.0 height=ROW_HEIGHT>
                                                <LabelledButton label={format!("Row {index}")} />
                                            </Frame>
                                        </Frame>
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Offset>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let row = rows[FOCUSED].get();
    assert!(harness.document().is_culled(row));

    let button = harness.document().focusables_within(row)[0];
    harness.document_mut().focus_focusable(button);
    harness.frame(Vec::new());

    assert!(
        !harness.document().is_culled(row),
        "the row holding the focus is laid out again"
    );
    assert!(
        text_within(harness.document(), row, &format!("Row {FOCUSED}")).is_some(),
        "what the focus moved onto is laid out"
    );
}
