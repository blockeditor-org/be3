use super::*;
use crate::reactive::view;

#[test]
fn a_clean_sibling_keeps_its_measurement_when_the_one_beside_it_changes() {
    let (steady, edited) = (NodeRef::new(), NodeRef::new());
    let mut document = build({
        let (steady, edited) = (steady.clone(), edited.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&steady string="steady" font_size=14.0 color=Color32::WHITE />
                    <Text @node_ref=&edited string="edited" font_size=14.0 color=Color32::WHITE />
                </List>
            }
        }
    });
    let (steady, edited) = (steady.get(), edited.get());
    let measures = counted_with_measures(&mut document, steady).measures;
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());
    let built = measures.get();
    assert!(built > 0, "the first frame has to measure the text");

    harness.frame(Vec::new());
    assert_eq!(
        measures.get(),
        built,
        "an unchanged frame must not measure anything again"
    );

    harness.document.set_text(edited, "edited twice");
    harness.frame(Vec::new());
    assert_eq!(
        measures.get(),
        built,
        "changing one child must not re-measure the child beside it"
    );

    harness.document.set_text(steady, "steady now");
    harness.frame(Vec::new());
    assert!(
        measures.get() > built,
        "changing a text has to re-measure it"
    );
}
