use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn the_click_that_ends_autoscroll_presses_nothing() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(100)}>|index: usize|
                        <LabelledButton
                            label={format!("Row {index}")}
                            on_click={move || clicks.update(|clicks| *clicks += 1)}
                        />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let origin = harness.rect(scroll).center();

    harness.frame(vec![Input::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.middle_button(origin, false);
    assert!(harness.document().autoscroll.is_some());

    harness.click(origin);

    assert!(harness.document().autoscroll.is_none());
    assert_eq!(clicks.get_untracked(), 0);

    harness.click(origin);

    assert_eq!(clicks.get_untracked(), 1);
}
