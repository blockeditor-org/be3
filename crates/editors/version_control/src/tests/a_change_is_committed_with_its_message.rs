use super::*;

#[test]
fn a_change_is_committed_with_its_message() {
    let note = Uuid::from_u128(9);
    let status = VersionStatus {
        branches: branches(),
        changes: vec![VersionChange {
            block_id: note.into_bytes(),
            block_type: TEXT.into_bytes(),
            name: Some("Note".into()),
            kind: VersionChangeKind::Modified,
        }],
        ..VersionStatus::default()
    };
    let state = Checkout {
        branch: "main".into(),
        ..Checkout::default()
    };
    let mut editor = checkout(state, status);

    editor.click("checkout.message");
    editor.text("Say more");
    editor.run();
    editor.click("checkout.commit");
    editor.run();

    assert_eq!(
        editor.take_version_commands(),
        vec![(
            own(&editor),
            VersionCommand::Commit {
                message: "Say more".into()
            }
        )]
    );
    editor.snapshot("a_change_is_committed_with_its_message");
}
