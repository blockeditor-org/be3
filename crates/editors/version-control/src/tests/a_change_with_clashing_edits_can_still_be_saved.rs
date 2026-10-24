use super::*;

#[test]
fn a_change_with_clashing_edits_can_still_be_saved() {
    let note = Uuid::from_u128(9);
    let state = Checkout {
        branch: "main".into(),
        conflicts: [CheckoutConflict {
            block: note,
            content_type: TEXT,
            kind: ConflictKind::Content,
            base: Some(Uuid::from_u128(10)),
            ours: Some(Uuid::from_u128(11)),
            theirs: Some(Uuid::from_u128(12)),
        }]
        .into_iter()
        .collect(),
        ..Checkout::default()
    };
    let status = VersionStatus {
        branches: branches(),
        changes: vec![VersionChange {
            block_id: note.into_bytes(),
            block_type: TEXT.into_bytes(),
            name: Some("Note".into()),
            kind: VersionChangeKind::Modified,
        }],
        working: Some(VersionCommit {
            conflicted: true,
            ..working()
        }),
        ..VersionStatus::default()
    };
    let mut editor = checkout(state, status);

    editor.click("checkout.description");
    editor.type_text("Brought in the other edits");
    editor.run();
    editor.click("checkout.save");
    editor.run();

    assert_eq!(
        editor.take_version_commands(),
        vec![(
            own(&editor),
            VersionCommand::Finish {
                description: "Brought in the other edits".into()
            }
        )]
    );
}
