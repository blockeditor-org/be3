use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn the_click_that_ends_autoscroll_presses_nothing() {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(100)}>
                        {move |index: usize| {
                            let counted = counted.clone();
                            view! {
                                <LabelledButton
                                    label={format!("Row {index}")}
                                    on_click={move || counted.set(counted.get() + 1)}
                                />
                            }
                        }}
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let origin = harness.rect(scroll).center();

    harness.frame(vec![Event::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.middle_button(origin, false);
    assert!(harness.document().autoscroll.is_some());

    harness.click(origin);

    assert!(harness.document().autoscroll.is_none());
    assert_eq!(clicks.get(), 0);

    harness.click(origin);

    assert_eq!(clicks.get(), 1);
}
