use super::*;
use crate::version_control::{MAIN_BRANCH, Repository, RepositoryContent};

#[test]
fn a_repository_moves_and_drops_branches() {
    let first = be_commit::CommitId::from_hash(be_store::Hash::of(b"first"));
    let upstream = Uuid::from_u128(7);
    let repository = Repository {
        upstream: Some(upstream),
        ..Repository::default()
    }
    .with_branch(MAIN_BRANCH, Some(first))
    .with_branch("draft", Some(first));

    let content = RepositoryContent::new(&repository.with_branch("draft", None));

    assert_eq!(content.root().branch(MAIN_BRANCH), Some(first));
    assert_eq!(content.root().branch("draft"), None);
    assert_eq!(content.references(), vec![upstream]);
    assert_eq!(RepositoryContent::decode(&content.encode()), Ok(content));
}
