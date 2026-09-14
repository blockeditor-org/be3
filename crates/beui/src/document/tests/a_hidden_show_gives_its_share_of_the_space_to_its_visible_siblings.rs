use super::*;
use crate::reactive::{build, view, Column, Frame, NodeRef, Show};

#[test]
fn a_hidden_show_gives_its_share_of_the_space_to_its_visible_siblings() {
    let shown = NodeRef::new();
    let document = build({
        let shown = shown.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Show @sizing=ItemSize::Percent(100.0) condition=false>
                        <Frame />
                    </Show>
                    <Show @sizing=ItemSize::Percent(100.0) @node_ref=&shown condition=true>
                        <Frame />
                    </Show>
                </Column>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(vec![]);

    let rect = harness.rect(shown.get());
    assert_eq!(rect.top(), 0.0);
    assert_eq!(rect.height(), VIEWPORT.y);
}
