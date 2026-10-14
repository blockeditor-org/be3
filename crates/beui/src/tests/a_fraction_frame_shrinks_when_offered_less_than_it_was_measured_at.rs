use super::*;
use crate::reactive::{Align, Frame, List, NodeRef, view, with_reactive_scope};

#[test]
fn a_fraction_frame_shrinks_when_offered_less_than_it_was_measured_at() {
    let _root = crate::reactive::RootScope::new();
    let quarter = NodeRef::new();
    let (width, set_width) = create_signal(Some(400.0));
    let document = build({
        move || {
            view! {
                <List align=Align::Start spacing=0.0>
                    <Frame width={width}>
                        <List align=Align::Start spacing=0.0>
                            <Frame @node_ref=&quarter width_fraction=0.25 height=10.0 />
                        </List>
                    </Frame>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(500.0, 200.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(quarter.get()).width(), 100.0);

    with_reactive_scope(harness.document_mut(), move || set_width.set(Some(360.0)));
    harness.frame(Vec::new());

    assert_eq!(harness.rect(quarter.get()).width(), 90.0);
}
