use super::*;

#[test]
fn ctrl_shift_i_opens_and_closes_the_inspector() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());
    assert!(!harness.inspector_open());

    harness.toggle_inspector();
    assert!(harness.inspector_open());

    harness.toggle_inspector();
    assert!(!harness.inspector_open());
}
