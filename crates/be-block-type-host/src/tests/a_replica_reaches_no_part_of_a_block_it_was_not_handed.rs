use super::*;

use crate::replicas::reach::{Reach, reaches};

/// A replica's module reads and reuses only the parts of its own block, of the content it was
/// handed and of the blocks that content is sealed in (a checkout copy's repository, say): a
/// part of another block named by its ref alone, as a module serving both, or a peer's edit,
/// could name it, is outside its reach.
#[test]
fn a_replica_reaches_no_part_of_a_block_it_was_not_handed() {
    with_test_types(|peer, _shared, _replicas| async move {
        let [mine, source, other] = [block(&peer).await, block(&peer).await, block(&peer).await];
        let put = |block: Uuid, bytes: &'static [u8]| {
            let peer = Arc::clone(&peer);
            async move { peer.put_part(block, bytes).await.expect("the part is put") }
        };
        let reach = Mutex::new(Reach::default());
        let reached = |part| reaches(&reach, &peer, mine, part);

        let peers = put(mine, b"a peer's part of this block").await;
        assert!(reached(peers).await.unwrap(), "a part sealed in its own block");
        let elsewhere = put(source, b"a part of another block").await;
        assert!(
            !reached(elsewhere).await.unwrap(),
            "a part of a block it was not handed"
        );

        reach.lock().unwrap().hand([elsewhere]);
        assert!(reached(elsewhere).await.unwrap(), "a part it was handed");
        let beside = put(source, b"another part of that block").await;
        assert!(
            reached(beside).await.unwrap(),
            "a part of a block what it was handed is sealed in"
        );
        let other = put(other, b"a part of a third block").await;
        assert!(
            !reached(other).await.unwrap(),
            "but not of a block it has nothing of"
        );
    });
}
