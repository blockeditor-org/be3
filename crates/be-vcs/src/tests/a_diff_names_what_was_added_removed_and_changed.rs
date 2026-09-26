use super::*;

#[test]
fn a_diff_names_what_was_added_removed_and_changed() {
    let vault = vault();
    let (kept, moved, removed, added) = (
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        Uuid::from_u128(4),
        Uuid::from_u128(5),
    );
    let base = tree([
        (ROOT, entry(None, manifest(&vault, "root"))),
        (kept, entry(Some(ROOT), manifest(&vault, "kept"))),
        (moved, entry(Some(ROOT), manifest(&vault, "moved"))),
        (removed, entry(Some(ROOT), manifest(&vault, "removed"))),
    ]);
    let mut current = base.clone();
    current.entries.get_mut(&moved).unwrap().parent = Some(kept);
    current.entries.get_mut(&kept).unwrap().metadata.name = Some("derived".into());
    current.entries.remove(&removed);
    current
        .entries
        .insert(added, entry(Some(ROOT), manifest(&vault, "added")));

    assert_eq!(
        diff(&base, &current),
        vec![
            Difference {
                block: moved,
                change: Change::Changed {
                    content: false,
                    placement: true,
                    metadata: false,
                },
            },
            Difference {
                block: removed,
                change: Change::Removed,
            },
            Difference {
                block: added,
                change: Change::Added,
            },
        ]
    );
}
