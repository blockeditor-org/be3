use super::*;
use crate::reactive::*;

#[test]
fn a_clean_sibling_keeps_its_measurement_when_the_one_beside_it_changes() {
    let _root = crate::reactive::RootScope::new();
    let (steady, edited) = (NodeRef::new(), NodeRef::new());
    let mut document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Text @node_ref=&steady color=Color32::WHITE>"steady"</Text>
                <Text @node_ref=&edited color=Color32::WHITE>"edited"</Text>
            </List>
        }
    });
    let (steady, edited) = (
        kind_of::<TextNode>(&document, steady.get()),
        kind_of::<TextNode>(&document, edited.get()),
    );
    let measures = counted_with_measures(&mut document, steady).measures;
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());
    let built = measures.get_untracked();
    assert!(built > 0, "the first frame has to measure the text");

    harness.frame(Vec::new());
    assert_eq!(
        measures.get_untracked(),
        built,
        "an unchanged frame must not measure anything again"
    );

    harness.document_mut().set_text(edited, "edited twice");
    harness.frame(Vec::new());
    assert_eq!(
        measures.get_untracked(),
        built,
        "changing one child must not re-measure the child beside it"
    );

    harness.document_mut().set_text(steady, "steady now");
    harness.frame(Vec::new());
    assert!(
        measures.get_untracked() > built,
        "changing a text has to re-measure it"
    );
}
