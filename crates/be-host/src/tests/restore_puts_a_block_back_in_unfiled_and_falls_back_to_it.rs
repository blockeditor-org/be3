use super::*;

const UNFILED: Uuid = Uuid::from_u128(0xf11e);
const FOLDER: Uuid = Uuid::from_u128(0xf01d);

fn restored(
    parent: Option<BlockParent>,
    deleted_from: Option<be_block_content::DeletedFrom>,
) -> RestoreTo {
    restore_to(
        &Restoring {
            parent,
            deleted_from,
            unfiled: Some(UNFILED),
            root_addable: false,
        },
        |parent| parent == UNFILED,
        |_| false,
    )
}

#[test]
fn restore_puts_a_block_back_in_unfiled_and_falls_back_to_it() {
    assert_eq!(
        restored(
            Some(BlockParent::Detached),
            Some(be_block_content::DeletedFrom::Block(UNFILED))
        ),
        RestoreTo::Unfiled(UNFILED),
        "a block deleted from Unfiled goes back there, though Unfiled lists nothing"
    );
    assert_eq!(
        restored(Some(BlockParent::Block(FOLDER)), None),
        RestoreTo::Unfiled(UNFILED),
        "a block whose folder is gone goes to Unfiled, not My files"
    );
    assert_eq!(
        restored(
            Some(BlockParent::Detached),
            Some(be_block_content::DeletedFrom::Root)
        ),
        RestoreTo::Unfiled(UNFILED),
        "so does a block that may not stand at the top"
    );
    assert_eq!(
        restore_to(
            &Restoring {
                parent: Some(BlockParent::Detached),
                deleted_from: None,
                unfiled: None,
                root_addable: false,
            },
            |_| true,
            |_| true,
        ),
        RestoreTo::Nowhere,
        "with no Unfiled yet, nothing moves"
    );
}
