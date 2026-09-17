use super::*;
use crate::reactive::{Frame, Row, build, view};

#[test]
fn a_list_sizes_plain_nodes_handed_to_it_intrinsically() {
    let (first, second) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (first, second) = (first.clone(), second.clone());
        move || {
            let children = vec![
                view! {
                    <Frame @node_ref=&first width=20.0 />
                },
                view! {
                    <Frame @node_ref=&second width=30.0 />
                },
            ];
            view! {
                <Row spacing=0.0 children />
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(first.get()).width(),
        20.0,
        "a list must give a plain node handed to it its intrinsic size"
    );
    assert_eq!(harness.rect(second.get()).width(), 30.0);
}
