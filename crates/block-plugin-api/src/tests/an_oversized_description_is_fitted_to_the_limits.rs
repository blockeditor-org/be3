use super::*;

#[test]
fn an_oversized_description_is_fitted_to_the_limits() {
    let node = DescribedNode {
        depth: 0,
        role: "TextInput".to_owned(),
        label: "é".repeat(MAX_STRING_BYTES),
        value: "x".repeat(MAX_STRING_BYTES + 1),
        toggled: None,
        disabled: false,
        focused: false,
        rect: None,
        actions: vec!["Focus".to_owned(), "y".repeat(MAX_STRING_BYTES + 1)],
    };
    let mut description = Description {
        nodes: vec![node; MAX_DESCRIBED_NODES + 1],
        test_ids: vec![
            TestIdRect {
                id: "z".repeat(MAX_STRING_BYTES + 1),
                rect: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
            };
            2
        ],
        actions: Vec::new(),
    };
    assert!(described(&description).is_err());

    description.fit();

    assert!(described(&description).is_ok());
    assert_eq!(description.nodes.len(), MAX_DESCRIBED_NODES);
    let first = &description.nodes[0];
    assert_eq!(first.label, "é".repeat(MAX_STRING_BYTES / 2));
    assert_eq!(first.value.len(), MAX_STRING_BYTES);
    assert_eq!(first.actions, ["Focus"]);
    assert!(description.test_ids.is_empty());
}
