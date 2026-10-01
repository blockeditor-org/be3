use super::*;
use crate::reactive::{Align, Frame, List, NodeRef, view};

#[test]
fn a_frame_takes_a_fraction_of_the_space_it_is_offered() {
    let quarter = NodeRef::new();
    let document = build({
        let quarter = quarter.clone();
        move || {
            view! {
                <List align=Align::Start spacing=0.0>
                    <Frame @node_ref=&quarter width_fraction=0.25 height=10.0 />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 200.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(quarter.get()).width(), 100.0);
}
