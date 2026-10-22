use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROWS: usize = 300;
const ROW_HEIGHT: f32 = 30.0;

#[test]
fn a_focused_node_scrolled_far_away_stays_laid_out() {
    let _root = crate::reactive::RootScope::new();
    let rows: Vec<NodeRef> = (0..ROWS).map(|_| NodeRef::new()).collect();
    let scroll = NodeRef::new();
    let document = build_node({
        let (rows, scroll) = (rows.clone(), scroll);
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <List spacing=0.0>
                            <ForEach keys={indices(ROWS)}>|index: usize|
                                <Frame @node_ref={&rows[index]} height=ROW_HEIGHT>
                                    <LabelledButton label={format!("Row {index}")} />
                                </Frame>
                            </ForEach>
                        </List>
                    </Offset>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(harness.document().focused_node().is_some());

    harness
        .document_mut()
        .set_scroll_offset(scroll.get(), ROWS as f32 * ROW_HEIGHT);
    harness.frame(Vec::new());

    let (first, second) = (rows[0].get(), rows[1].get());
    assert!(
        harness.document().is_culled(second),
        "the rows scrolled far past are culled"
    );
    assert!(
        !harness.document().is_culled(first),
        "the row holding the focus is not"
    );
    assert!(
        text_within(harness.document(), first, "Row 0").is_some(),
        "what the focus is on stays laid out"
    );
}
