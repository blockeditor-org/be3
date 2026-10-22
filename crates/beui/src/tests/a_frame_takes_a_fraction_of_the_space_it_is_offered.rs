use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_frame_takes_a_fraction_of_the_space_it_is_offered() {
    let _root = crate::reactive::RootScope::new();
    let quarter = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List align=Align::Start spacing=0.0>
                <Frame @node_ref=&quarter width={Extent::fraction(0.25)} height=10.0 />
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 200.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(quarter.get()).width(), 100.0);
}
