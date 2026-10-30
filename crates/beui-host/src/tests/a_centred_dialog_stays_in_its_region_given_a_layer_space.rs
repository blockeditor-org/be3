use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_centred_dialog_stays_in_its_region_given_a_layer_space() {
    let _root = crate::reactive::RootScope::new();
    let surface = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog open=true kind=DialogKind::Alert title="Delete?" on_dismiss={|| {}}>
                    <Text color=Color32::WHITE @node_ref={&surface}>"Body"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    give_layer_space(&mut harness);
    harness.frame(Vec::new());

    assert!(
        harness.document().escaped_layers().is_empty(),
        "an alert centred in its region is no layer of its own"
    );
    let body = harness.rect(surface.get());
    assert!((body.center().x - VIEWPORT.x / 2.0).abs() < 2.0);
}
