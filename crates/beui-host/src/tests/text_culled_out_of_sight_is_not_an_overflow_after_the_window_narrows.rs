use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::styled::{Scroll, Text};

#[test]
fn text_culled_out_of_sight_is_not_an_overflow_after_the_window_narrows() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=0.0>
                        <Frame height=3000.0 />
                        <Text @node_ref=&text>
                            "a line long enough to wrap once the window is narrower than it"
                        </Text>
                    </List>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.resize(Vec2::new(200.0, WIDE_VIEWPORT.y));
    harness.frame(Vec::new());

    assert!(
        harness.document().is_culled(text.get()),
        "the text is out of sight"
    );
    let found: Vec<String> = harness
        .document()
        .overflows()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}
