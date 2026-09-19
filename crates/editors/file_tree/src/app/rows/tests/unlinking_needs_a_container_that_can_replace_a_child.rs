use super::*;

#[test]
fn unlinking_needs_a_container_that_can_replace_a_child() {
    let client = client();
    let block_types = block_types();

    let refusing = catalog(ChildEdits::default());
    assert_eq!(
        unlink_permission(&client, &refusing, &block_types, Some(CONTAINER)),
        Err("This container doesn't support replacing a reference")
    );
    assert_eq!(
        unlink_permission(&client, &refusing, &block_types, None),
        Err("Loading\u{2026}"),
        "a row with no container is not a reference held anywhere"
    );

    let replacing = catalog(ChildEdits {
        add: false,
        delete: false,
        replace: true,
    });
    assert_eq!(
        unlink_permission(&client, &replacing, &block_types, Some(CONTAINER)),
        Ok(())
    );
}
