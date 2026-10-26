use super::*;

fn counted_preview(preview: Option<Result<Uuid, String>>) -> Option<i64> {
    let preview = preview?.ok()?;
    content(preview).map(|content| {
        CounterContent::decode(content.bytes())
            .unwrap()
            .root()
            .value()
    })
}

#[test]
fn a_merge_conflict_shows_both_versions_and_keeps_the_one_chosen() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 5);
    wait_for_count(block, 5);
    flush();
    let ours = node(block).unwrap().head.unwrap();
    add(block, 2);
    wait_for_count(block, 7);
    flush();
    let theirs = node(block).unwrap().head.unwrap();
    with_shared_mut(|shared| {
        shared
            .conflicts
            .entry(block)
            .or_default()
            .push(be_types_host::Conflict {
                ours,
                theirs,
                count: 1,
            });
    });
    assert_eq!(
        conflicts::conflicted(block).map(|conflict| conflict.count),
        Some(1)
    );

    conflicts::compare(block);
    wait_until("both versions are shown", |shared| {
        shared
            .comparison
            .as_ref()
            .is_some_and(|comparison| comparison.ours.is_some() && comparison.theirs.is_some())
    });
    let comparison = conflicts::comparing().unwrap();
    assert_eq!(counted_preview(comparison.ours.clone()), Some(5));
    assert_eq!(counted_preview(comparison.theirs.clone()), Some(7));

    conflicts::keep(block, conflicts::Side::Ours);
    wait_for_count(block, 5);
    wait_until("the conflict is resolved", |shared| {
        !shared.conflicts.contains_key(&block) && shared.comparison.is_none()
    });
    revisions::collect_released();
    for preview in [comparison.ours, comparison.theirs] {
        assert!(
            content(preview.unwrap().unwrap()).is_none(),
            "the previews are let go once the conflict is resolved"
        );
    }
}
