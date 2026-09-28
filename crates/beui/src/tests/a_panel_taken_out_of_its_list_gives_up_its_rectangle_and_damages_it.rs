use super::*;

#[test]
fn a_panel_taken_out_of_its_list_gives_up_its_rectangle_and_damages_it() {
    let panels = three_panels();
    let (list, middle) = (panels.list, panels.middle);
    let mut harness = Harness::new(panels.document);
    harness.frame(Vec::new());
    let vacated = harness.rect(middle);

    harness.document_mut().remove_child(list, middle);
    let output = harness.frame(Vec::new());

    assert!(
        harness.document().node_rect(middle).is_none(),
        "a node its parent stopped placing is no longer placed"
    );
    let damage = output.damage().expect("the list closed the gap");
    assert!(damage.intersects(vacated));
}
