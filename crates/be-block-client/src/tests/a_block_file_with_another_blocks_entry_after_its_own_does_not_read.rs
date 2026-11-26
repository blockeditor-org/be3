use super::*;

/// A block file's log holds history written in other blocks, then what was written in its
/// block: one carrying an entry written in another block after one of its own, which would be
/// replayed as history the block never had, does not read.
#[test]
fn a_block_file_with_another_blocks_entry_after_its_own_does_not_read() {
    let block = Uuid::from_u128(0xb10c);
    let elsewhere = Uuid::from_u128(0xe15e);
    let entry = |written_in: Uuid| crate::ExportedEntry {
        block: written_in,
        seq: 0,
        id: Uuid::from_u128(1),
        author: Uuid::from_u128(2),
        arrival: 0,
        said: None,
        signature: vec![0; 64],
    };
    let file = |written_in: &[Uuid]| {
        crate::transfer::encode(&[crate::transfer::SavedBlock {
            id: block,
            block_type: be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            parent: None,
            metadata: be_block_content::BlockMetadata::named("Chat"),
            references: Vec::new(),
            content: None,
            log: written_in.iter().map(|block| entry(*block)).collect(),
        }])
    };
    assert_eq!(
        crate::transfer::decode(&file(&[elsewhere, block])).unwrap()[0]
            .log
            .len(),
        2,
        "history from another block leads the block's own entries"
    );
    assert!(
        crate::transfer::decode(&file(&[block, elsewhere])).is_err(),
        "an entry of another block's after the block's own is refused"
    );
}
