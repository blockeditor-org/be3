use uuid::Uuid;

use super::*;

/// A block whose local edits were discarded keeps none until the server says this account may
/// edit it again (a join's `may-edit`, `may_edit_again`): a listing that says so may be older
/// than the access that lost it.
#[test]
fn a_discarded_blocks_edits_are_kept_again_only_on_the_servers_word() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    let block = Uuid::new_v4();
    refs.drop_local_edits(block).unwrap();

    let editable = BlockSummary::no_access(block, Uuid::new_v4(), be_block_graph::Access::Edit);
    refs.note(&editable).unwrap();
    assert!(refs.discarded(block), "a listing is not the server's word");
    assert!(!refs.keep_open_edits(block, OpenEdits::default()).unwrap());

    refs.may_edit_again(block).unwrap();
    assert!(!refs.discarded(block));
    assert!(refs.keep_open_edits(block, OpenEdits::default()).unwrap());

    let reopened = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    assert!(!reopened.discarded(block), "cleared for good");
}
