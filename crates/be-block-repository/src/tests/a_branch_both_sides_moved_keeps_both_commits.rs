use super::*;
use crate::{MAIN_BRANCH, Repository, RepositoryContent};
use be_block_commit::{CommitId, MergeResult};

fn commit(name: &str) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name.as_bytes()))
}

fn merged(
    base: &Repository,
    ours: &Repository,
    theirs: &Repository,
) -> MergeResult<RepositoryContent> {
    RepositoryContent::merge3(
        &RepositoryContent::new(base),
        &RepositoryContent::new(ours),
        &RepositoryContent::new(theirs),
    )
}

#[test]
fn a_branch_both_sides_moved_keeps_both_commits() {
    let base = Repository::default()
        .with_branch(MAIN_BRANCH, Some(commit("base")))
        .with_branch("draft", Some(commit("base")));
    let ours = base
        .with_branch(MAIN_BRANCH, Some(commit("ours")))
        .with_branch("draft", Some(commit("draft")));
    let theirs = base.with_branch(MAIN_BRANCH, Some(commit("theirs")));

    let MergeResult::Clean(content) = merged(&base, &ours, &theirs) else {
        panic!("a branch both sides moved conflicted");
    };
    let root = content.root();
    assert_eq!(root.branch(MAIN_BRANCH), Some(commit("theirs")));
    assert_eq!(root.branch("main-2"), Some(commit("ours")));
    assert_eq!(
        root.branch("draft"),
        Some(commit("draft")),
        "one side's move"
    );

    let again = root.with_branch(MAIN_BRANCH, Some(commit("ours, again")));
    let later = root.with_branch(MAIN_BRANCH, Some(commit("theirs, again")));
    let MergeResult::Clean(content) = merged(&root, &again, &later) else {
        panic!("a branch both sides moved conflicted");
    };
    let root = content.root();
    assert_eq!(root.branch(MAIN_BRANCH), Some(commit("theirs, again")));
    assert_eq!(root.branch("main-2"), Some(commit("ours")));
    assert_eq!(root.branch("main-3"), Some(commit("ours, again")));

    let same = base.with_branch(MAIN_BRANCH, Some(commit("same")));
    let MergeResult::Clean(content) = merged(&base, &same, &same) else {
        panic!("a branch moved to the same commit conflicted");
    };
    assert_eq!(content.root().branches.len(), 2);
}
