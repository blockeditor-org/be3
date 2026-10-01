use super::*;

#[test]
fn a_removed_nodes_slot_is_reused_under_a_new_id() {
    let panels = three_panels();
    let (list, middle) = (panels.list, panels.middle);
    let mut harness = Harness::new(panels.document);
    harness.frame(Vec::new());
    let vacated = harness.rect(middle);

    harness.document_mut().remove_child(list, middle.id());
    harness.document_mut().remove_node(middle.id());
    harness.frame(Vec::new());
    let document = harness.document_mut();
    let replacement = document.create_frame();
    document.set_frame_height(replacement, Some(100.0));
    document.append_child(list, replacement.id(), ItemSize::Intrinsic);
    harness.frame(Vec::new());

    assert_eq!(
        replacement.id().index(),
        middle.id().index(),
        "a node made after a frame takes the slot a removed node gave up"
    );
    assert_ne!(
        replacement.id(),
        middle.id(),
        "the reused slot hands out a new id"
    );
    assert!(
        !harness.document().contains(middle),
        "the removed node's id no longer names a node"
    );
    assert!(
        harness.document().node_rect(middle).is_none(),
        "the removed node's id reads nothing the new node was given"
    );
    assert_eq!(
        harness.rect(replacement).height(),
        vacated.height(),
        "the new node is laid out in its own right"
    );
}
