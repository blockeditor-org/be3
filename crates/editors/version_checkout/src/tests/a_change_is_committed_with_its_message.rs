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
    let checkout = Checkout {
        branch: "main".into(),
        ..Checkout::default()
    };
    let mut harness = harness(checkout, status);

    harness.editor.click("checkout.message");
    harness.editor.text("Say more");
    harness.run();
    harness.editor.click("checkout.commit");
    harness.run();

    assert_eq!(
        harness.host.take_version_commands(),
        vec![(
            harness.editor.block_id().expect("the editor has a block"),
            VersionCommand::Commit {
                message: "Say more".into()
            }
        )]
    );
    harness
        .editor
        .snapshot("a_change_is_committed_with_its_message");
}
