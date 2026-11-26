use crate::version::{COMMITS_KEPT, Objects, TREES_KEPT};

use be_block_commit::{Commit, CommitId};
use be_block_store::{Hash, Manifest};
use be_block_vcs::Tree;
use uuid::Uuid;

fn id(n: usize) -> CommitId {
    CommitId::from_hash(Hash::of(&n.to_le_bytes()))
}

#[test]
fn the_version_cache_keeps_only_the_trees_and_commits_used_last() {
    let objects = Objects::default();
    let commit = Commit::new(Manifest::default(), Uuid::nil(), 0);
    objects.put_tree(id(0), Tree::default());
    objects.put_commit(id(0), commit.clone());
    for n in 1..COMMITS_KEPT * 2 {
        // The first is used all along, so it is never the one let go.
        assert!(
            objects.tree(id(0)).is_some(),
            "the tree used all along is kept"
        );
        assert!(
            objects.commit(id(0)).is_some(),
            "the commit used all along is kept"
        );
        objects.put_tree(id(n), Tree::default());
        objects.put_commit(id(n), commit.clone());
    }
    assert_eq!(
        objects.kept(),
        (TREES_KEPT, COMMITS_KEPT),
        "it stays bounded"
    );
    assert!(
        objects.tree(id(1)).is_none(),
        "the tree used longest ago is let go"
    );
    assert!(
        objects.commit(id(1)).is_none(),
        "the commit used longest ago is let go"
    );
    let last = COMMITS_KEPT * 2 - 1;
    assert!(objects.tree(id(last)).is_some() && objects.commit(id(last)).is_some());
}
