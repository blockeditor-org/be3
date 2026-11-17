use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;
use crate::unstyled::{TOOLTIP_COOL_DOWN, TOOLTIP_DELAY, Tooltip, TooltipHandle};

#[test]
fn tooltips_share_a_warm_up_that_cools_down_after_the_last_one_closes() {
    let _root = crate::reactive::RootScope::new();
    let first = NodeRef::new();
    let second = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=40.0>
                <Tooltip
                    label="First"
                    content={move |handle: TooltipHandle| view! {
                        <Text @test_id="first.tip">{handle.label}</Text>
                    }}
                >
                    <Button @node_ref=&first>
                        <Text>"one"</Text>
                    </Button>
                </Tooltip>
                <Tooltip
                    label="Second"
                    content={move |handle: TooltipHandle| view! {
                        <Text @test_id="second.tip">{handle.label}</Text>
                    }}
                >
                    <Button @node_ref=&second>
                        <Text>"two"</Text>
                    </Button>
                </Tooltip>
            </List>
        }
    });
    let (first, second) = (first.get(), second.get());
    let mut harness = Harness::sized(document, Vec2::new(300.0, 300.0));
    harness.frame(Vec::new());
    let shown = |harness: &Harness, id: &str| harness.document().find_test_id(id).is_some();

    harness.frame(vec![Input::PointerMoved(harness.center(first))]);
    harness.frame(Vec::new());
    assert!(!shown(&harness, "first.tip"), "a cold tooltip waits its delay");
    harness.advance(TOOLTIP_DELAY);
    harness.frame(Vec::new());
    assert!(shown(&harness, "first.tip"));

    harness.frame(vec![Input::PointerMoved(harness.center(second))]);
    harness.frame(Vec::new());
    assert!(!shown(&harness, "first.tip"));
    assert!(
        shown(&harness, "second.tip"),
        "the next tooltip opens at once while the first is warm"
    );

    let away = Pos2::new(290.0, 290.0);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.frame(Vec::new());
    assert!(!shown(&harness, "second.tip"));
    harness.advance(TOOLTIP_COOL_DOWN / 2);
    harness.frame(vec![Input::PointerMoved(harness.center(first))]);
    harness.frame(Vec::new());
    assert!(
        shown(&harness, "first.tip"),
        "within the cool-down the warm-up holds"
    );

    harness.frame(vec![Input::PointerMoved(away)]);
    harness.frame(Vec::new());
    harness.advance(TOOLTIP_COOL_DOWN);
    harness.frame(vec![Input::PointerMoved(harness.center(second))]);
    harness.frame(Vec::new());
    assert!(
        !shown(&harness, "second.tip"),
        "past the cool-down a tooltip waits its delay again"
    );
    harness.advance(TOOLTIP_DELAY);
    harness.frame(Vec::new());
    assert!(shown(&harness, "second.tip"));
}
