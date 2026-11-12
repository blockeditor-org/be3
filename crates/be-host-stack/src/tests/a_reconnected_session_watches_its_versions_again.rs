use super::*;
use crate::version::Versions;

#[test]
fn a_reconnected_session_watches_its_versions_again() {
    let mut versions = Versions::default();
    let block = Uuid::now_v7();
    assert!(versions.note_watching(block), "a block is watched once");
    assert!(
        !versions.note_watching(block),
        "a watch already taken is not taken twice"
    );
    versions.reconnected();
    assert!(
        versions.note_watching(block),
        "the new connection watches nothing yet, so the block is watched again"
    );
}
