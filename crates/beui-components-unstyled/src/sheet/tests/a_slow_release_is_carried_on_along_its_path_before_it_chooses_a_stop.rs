use super::*;

#[test]
fn a_slow_release_is_carried_on_along_its_path_before_it_chooses_a_stop() {
    let _root = RootScope::new();
    let node = NodeRef::new();
    node.fill(NodeId::new(1, 0));
    let stops = [(200.0, "low"), (400.0, "high")];
    assert_eq!(
        snap_to(node, 310.0, 0.0, stops),
        Some("high"),
        "let go still just above the midpoint, the sheet rises to the higher stop"
    );
    assert_eq!(
        snap_to(node, 310.0, -300.0, stops),
        Some("low"),
        "let go there falling slowly, it is carried past the midpoint to the lower one"
    );
}
