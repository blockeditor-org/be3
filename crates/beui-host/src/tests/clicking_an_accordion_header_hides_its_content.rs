use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn clicking_an_accordion_header_hides_its_content() {
    let _root = crate::reactive::RootScope::new();
    let body = NodeRef::new();
    let (document, [accordion]) = toolbar_of(move || {
        [view! {
            <Accordion title="About" open=true>
                <Text @node_ref=&body color=Color32::WHITE>
                    "beui keeps a retained tree of nodes."
                </Text>
            </Accordion>
        }]
    });
    let body = body.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert!(harness.document().node_rect(body).is_some());

    let header = harness.rect(accordion);
    harness.click(pos2(header.center().x, header.top() + 8.0));
    harness.frame(Vec::new());

    assert!(
        !harness
            .document()
            .api::<styled::DisclosureApi>(accordion)
            .open()
    );
    assert!(harness.document().node_rect(body).is_none());
}
