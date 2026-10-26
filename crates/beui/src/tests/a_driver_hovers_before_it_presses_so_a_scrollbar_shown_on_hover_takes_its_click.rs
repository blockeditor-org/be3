use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::*;

#[test]
fn a_driver_hovers_before_it_presses_so_a_scrollbar_shown_on_hover_takes_its_click() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=2000.0 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let scroll = scroll.get();
    let bar = harness.rect(harness.document().children(scrollbar_of(&harness, scroll))[0]);
    let below_the_thumb = pos2(bar.center().x, bar.bottom() - 2.0);

    harness.run(&[
        "app",
        "click",
        &format!("{},{}", below_the_thumb.x, below_the_thumb.y),
    ]);

    assert_eq!(
        harness.document().scroll_offset(scroll),
        VIEWPORT.y,
        "a click on the track below the thumb pages down"
    );
}
