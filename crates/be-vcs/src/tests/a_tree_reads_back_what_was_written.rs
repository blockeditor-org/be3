use super::*;

#[test]
fn a_tree_reads_back_what_was_written() {
    let vault = vault();
    let child = Uuid::from_u128(2);
    let written = tree([
        (ROOT, entry(None, manifest(&vault, "folder"))),
        (child, entry(Some(ROOT), manifest(&vault, "notes"))),
    ]);

    let stored = written.write(&vault).unwrap();

    assert_eq!(Tree::read(&vault, &stored).unwrap(), written);
    assert_eq!(written.children(ROOT).collect::<Vec<_>>(), vec![child]);
    assert_eq!(
        stored,
        written.write(&vault).unwrap(),
        "the same tree was stored twice"
    );
}
