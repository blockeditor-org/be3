use super::*;
use crate::reactive::{Align, Direction, Frame, ItemSize, List, NodeRef, view};

#[test]
fn one_child_can_align_itself_apart_from_its_row() {
    let (tall, centred, ended) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (tall, centred, ended) = (tall.clone(), centred.clone(), ended.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal align=Align::Start spacing=0.0>
                    <Frame @node_ref=&tall width=10.0 height=40.0 />
                    <Frame
                        @sizing={ItemSize::Intrinsic.align(Align::Center)}
                        @node_ref=&centred
                        width=10.0
                        height=10.0
                    />
                    <Frame
                        @sizing={ItemSize::Intrinsic.align(Align::End)}
                        @node_ref=&ended
                        width=10.0
                        height=10.0
                    />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 40.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(tall.get()).top(), 0.0);
    assert_eq!(harness.rect(centred.get()).top(), 15.0);
    assert_eq!(harness.rect(ended.get()).top(), 30.0);
}
