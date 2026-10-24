use super::*;

#[test]
fn a_change_is_saved_with_its_description() {
    let note = Uuid::from_u128(9);
    let status = VersionStatus {
        branches: branches(),
        changes: vec![VersionChange {
            block_id: note.into_bytes(),
            block_type: TEXT.into_bytes(),
            name: Some("Note".into()),
            kind: VersionChangeKind::Modified,
        }],
        working: Some(working()),
        log: history(),
        ..VersionStatus::default()
    };
    let state = Checkout {
        branch: "main".into(),
        ..Checkout::default()
    };
    let mut editor = checkout(state, status);

    editor.click("checkout.description");
    editor.type_text("Say more");
    editor.run();
    editor.click("checkout.save");
    editor.run();

    assert_eq!(
        editor.take_version_commands(),
        vec![(
            own(&editor),
            VersionCommand::Finish {
                description: "Say more".into()
            }
        )]
    );
    editor.snapshot("a_change_is_saved_with_its_description");
}
