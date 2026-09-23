use super::*;

#[test]
fn two_checklists_conflict_only_where_both_changed_one_field() {
    let base = listed(&ChecklistContent::default(), &[ChecklistOp::add("milk")]);
    let milk = base.items()[0].id;
    let ours = listed(
        &base,
        &[ChecklistOp::SetText {
            id: milk,
            text: "whole milk".into(),
        }],
    );
    let theirs = listed(
        &base,
        &[
            ChecklistOp::SetText {
                id: milk,
                text: "oat milk".into(),
            },
            ChecklistOp::SetDone {
                id: milk,
                done: true,
            },
        ],
    );

    let merged = ChecklistContent::merge3(&base, &ours, &theirs);

    assert_eq!(
        merged,
        MergeResult::Conflicted {
            value: listed(
                &base,
                &[
                    ChecklistOp::SetText {
                        id: milk,
                        text: "whole milk".into(),
                    },
                    ChecklistOp::SetDone {
                        id: milk,
                        done: true
                    },
                ],
            ),
            conflicts: 1,
        }
    );
}
