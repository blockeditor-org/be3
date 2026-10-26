use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn rows_removed_under_a_scroll_at_its_end_leave_it_at_the_new_end() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let rows = create_signal(indices(300));
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={rows}>|index: usize|
                        <LabelledButton
                            @test_id={format!("row.{index}")}
                            label={format!("Row {index}")}
                            on_click={|| {}}
                        />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let center = harness.rect(scroll).center();
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(
        0.0, -100_000.0,
    )))]);
    let end = harness.document().scroll_offset(scroll);
    assert!(end > 0.0);

    rows.set(indices(40));
    harness.frame(Vec::new());
    let offset = harness.document().scroll_offset(scroll);
    assert!(offset > 0.0 && offset < end, "{offset} after {end}");
    let last = harness.find("row.39");
    assert_eq!(
        harness.rect(last).bottom().round(),
        harness.rect(scroll).bottom().round(),
        "the list ends at the bottom of the view in the frame the rows go"
    );
    assert_eq!(harness.document().scroll_overscroll(scroll), 0.0);
    assert!(
        !harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating(),
        "it is put there, not sprung back"
    );
}
