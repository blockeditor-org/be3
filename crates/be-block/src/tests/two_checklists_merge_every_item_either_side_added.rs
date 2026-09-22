use super::*;

#[test]
fn two_checklists_merge_every_item_either_side_added() {
    let base = listed(
        &ChecklistContent::default(),
        &[ChecklistOp::add("milk"), ChecklistOp::add("eggs")],
    );
    let milk = base.items()[0].id;
    let eggs = base.items()[1].id;
    let ours = listed(
        &base,
        &[
            ChecklistOp::add("bread"),
            ChecklistOp::SetDone {
                id: milk,
                done: true,
            },
        ],
    );
    let theirs = listed(
        &base,
        &[
            ChecklistOp::add("jam"),
            ChecklistOp::SetText {
                id: milk,
                text: "oat milk".into(),
            },
            ChecklistOp::Remove { id: eggs },
        ],
    );

    let MergeResult::Clean(merged) = ChecklistContent::merge3(&base, &ours, &theirs) else {
        panic!("the two sides changed different things");
    };

    assert_eq!(
        texts(&merged),
        [("oat milk", true), ("bread", false), ("jam", false)]
    );
}
