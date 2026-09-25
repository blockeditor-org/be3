use super::*;

#[test]
fn a_conflict_is_resolved_by_taking_a_side() {
    let note = Uuid::from_u128(9);
    let checkout = Checkout {
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
        behind: true,
        ..VersionStatus::default()
    };
    let mut harness = harness(checkout, status);
    harness
        .editor
        .snapshot("a_conflict_is_resolved_by_taking_a_side");

    harness.editor.click(&format!("checkout.theirs.{note}"));
    harness.run();

    assert_eq!(
        harness.host.take_version_commands(),
        vec![(
            harness.editor.block_id().expect("the editor has a block"),
            VersionCommand::Resolve {
                block_id: note.into_bytes(),
                take: ConflictSide::Theirs,
            }
        )]
    );
}
