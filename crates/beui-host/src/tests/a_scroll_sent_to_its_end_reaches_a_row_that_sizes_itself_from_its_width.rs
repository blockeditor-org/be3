use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Container, Scroll, ScrollApi};

#[test]
fn a_scroll_sent_to_its_end_reaches_a_row_that_sizes_itself_from_its_width() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let api = ComponentRef::<ScrollApi>::new();
    let rows = create_signal((0..10).collect::<Vec<usize>>());
    let document = build_node(move || {
        view! {
            <Scroll @node_ref=&scroll api>
                <List spacing=0.0>
                    <ForEach keys={rows}>|_row: usize|
                        <Container>|size: ReadSignal<Vec2>| {
                            let height = create_memo(move || match size.get().x < 300.0 {
                                true => 120.0,
                                false => 40.0,
                            });
                            view! {
                                <Frame height fill=Color32::WHITE />
                            }
                        }</Container>
                    </ForEach>
                </List>
            </Scroll>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, vec2(250.0, 300.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    crate::native::with_reactive_scope(harness.document_mut(), || {
        rows.update(|rows| rows.push(rows.len()));
        api.get().scroll_to_end();
    });
    harness.frame(Vec::new());
    let document = harness.document();
    let offset = document
        .first_offset_within(scroll)
        .expect("a scroll lays its content out in an offset");
    let position = document
        .offset_position(offset)
        .expect("a laid-out offset knows its position");
    assert_eq!(position.content, 11.0 * 120.0);
    assert_eq!(
        document.offset_value(offset),
        position.max_offset(),
        "the scroll ends at the end of the row it was sent to, at the height that row settled on"
    );
}
