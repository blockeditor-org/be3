use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_fraction_frame_shrinks_when_offered_less_than_it_was_measured_at() {
    let _root = crate::reactive::RootScope::new();
    let quarter = NodeRef::new();
    let width = create_signal(Some(400.0));
    let document = build_node(move || {
        view! {
            <List align=Align::Start spacing=0.0>
                <Frame width=width>
                    <List align=Align::Start spacing=0.0>
                        <Frame @node_ref=&quarter width={Extent::fraction(0.25)} height=10.0 />
                    </List>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(500.0, 200.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(quarter.get()).width(), 100.0);

    width.set(Some(360.0));
    harness.frame(Vec::new());

    assert_eq!(harness.rect(quarter.get()).width(), 90.0);
}
