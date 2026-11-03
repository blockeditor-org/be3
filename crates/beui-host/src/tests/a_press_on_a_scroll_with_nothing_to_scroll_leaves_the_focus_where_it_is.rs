use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_press_on_a_scroll_with_nothing_to_scroll_leaves_the_focus_where_it_is() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll>
                    <List spacing=0.0>
                        <Interactive @test_id="field" focusable=true>
                            <Frame width=60.0 height=20.0 />
                        </Interactive>
                        <Frame @test_id="blank" width=60.0 height=40.0 />
                    </List>
                </Scroll>
                <Frame @test_id="box" height=100.0>
                    <Scroll>
                        <Frame width=60.0 height=400.0 />
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let field = harness.find("field");
    let blank = harness.find("blank");
    let scrolling = harness.find("box");

    harness.click(harness.center(field));
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused_node(), Some(field));

    harness.click(harness.center(blank));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "a press on a scroll with nothing to scroll keeps the field focused"
    );

    harness.click(harness.center(scrolling));
    harness.frame(Vec::new());
    assert_ne!(
        harness.document().focused_node(),
        Some(field),
        "a scroll that scrolls takes the focus, so its keys scroll it"
    );
}
