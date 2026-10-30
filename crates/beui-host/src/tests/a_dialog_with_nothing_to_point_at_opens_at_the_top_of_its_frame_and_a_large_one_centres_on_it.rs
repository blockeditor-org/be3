use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_dialog_with_nothing_to_point_at_opens_at_the_top_of_its_frame_and_a_large_one_centres_on_it() {
    let _root = crate::reactive::RootScope::new();
    let small = create_signal(false);
    let large = create_signal(false);
    let (small_body, large_body) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog
                    open={small}
                    title="Find"
                    anchor={DialogAnchor::Rect(None)}
                    width={Size::Custom(200.0)}
                    on_dismiss={move || small.set(false)}
                >
                    <Frame @node_ref=&small_body height=40.0 />
                </Dialog>
                <Dialog
                    open={large}
                    title="Share"
                    size=DialogSize::Large
                    anchor={DialogAnchor::Rect(None)}
                    width={Size::Custom(300.0)}
                    on_dismiss={move || large.set(false)}
                >
                    <Frame @node_ref=&large_body height=40.0 />
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    small.set(true);
    harness.settle();
    let body = harness.rect(small_body.get());
    assert!(
        body.top() < WIDE_VIEWPORT.y / 3.0 && (body.center().x - WIDE_VIEWPORT.x / 2.0).abs() < 2.0,
        "a dialog with nothing to point at opens at the top of its frame: {body:?}"
    );
    give_layer_space(&mut harness);
    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1, "it escapes as one card");
    assert!(layers[0].scrims.is_empty(), "dimming nothing");
    assert!(layers[0].dismiss_on_outside_press && layers[0].blocks_owner);
    harness.context().set_layer_space(WindowId::MAIN, None);
    small.set(false);
    harness.settle();

    large.set(true);
    harness.settle();
    let body = harness.rect(large_body.get());
    assert!(
        (body.center().x - WIDE_VIEWPORT.x / 2.0).abs() < 2.0 && body.top() > WIDE_VIEWPORT.y / 3.0,
        "a large dialog centres on its frame: {body:?}"
    );
    give_layer_space(&mut harness);
    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1, "it escapes, attached to its frame");
    assert_eq!(layers[0].scrims.len(), 1, "and dims its frame");
}
