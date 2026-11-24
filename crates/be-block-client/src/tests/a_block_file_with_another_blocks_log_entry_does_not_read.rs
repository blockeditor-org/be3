use super::*;

/// A block file's log holds what was written in its block: one carrying an entry written in
/// another block, which would be replayed as history the block never had, does not read.
#[test]
fn a_block_file_with_another_blocks_log_entry_does_not_read() {
    let block = Uuid::from_u128(0xb10c);
    let entry = |written_in: Uuid| crate::ExportedEntry {
        block: written_in,
        seq: 0,
        id: Uuid::from_u128(1),
        author: Uuid::from_u128(2),
        arrival: 0,
        said: None,
        signature: vec![0; 64],
    };
    let file = |written_in: Uuid| {
        crate::transfer::encode(&[crate::transfer::SavedBlock {
            id: block,
            block_type: be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            parent: None,
            metadata: be_block_content::BlockMetadata::named("Chat"),
            references: Vec::new(),
            content: None,
            log: vec![entry(written_in)],
        }])
    };
    assert_eq!(
        crate::transfer::decode(&file(block)).unwrap()[0].log.len(),
        1
    );
    assert!(
        crate::transfer::decode(&file(Uuid::from_u128(0xe15e))).is_err(),
        "an entry of another block's is refused"
    );
}
