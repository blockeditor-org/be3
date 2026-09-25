use super::{
    CommitId, MAIN_BRANCH, VersionControlData, VersionControlDataOperation, apply, author,
};

#[test]
fn version_control_data_set_branch_ignores_unknown_commit() {
    let mut data = VersionControlData::new(author(), 1_000);
    let initial = data.branch_head(MAIN_BRANCH).cloned().unwrap();

    apply(
        &mut data,
        VersionControlDataOperation::SetBranch {
            name: MAIN_BRANCH.to_owned(),
            expected: Some(initial.clone()),
            commit: CommitId::default(),
        },
    );

    assert_eq!(data.branch_head(MAIN_BRANCH), Some(&initial));
}
