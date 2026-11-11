use be_sdk_beui::beui::ClipItem;
use be_sdk_beui::beui::clipboard::{Blocks, formats};
use be_sdk_beui::{BlockInfo, BlockParent};
use be_sdk_test::HostCall;

use super::*;

#[test]
fn copy_and_cut_put_a_block_reference_on_the_clipboard() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let notes = Uuid::from_u128(5);
    let kind = Uuid::from_u128(1);
    let mut listed = BlockInfo::new(notes, kind, BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    my_files(&mut fixture, vec![listed]);
    fixture.settle();

    fixture.choose(notes, "Copy");
    let copied = fixture.test.take_copied(formats::BLOCKS);
    let [copied] = &copied[..] else {
        panic!("Copy puts the block on the clipboard once, not {copied:?}");
    };
    let blocks = ClipItem::new(formats::BLOCKS, copied.clone())
        .decode::<Blocks>()
        .expect("the block reference decodes");
    let [block] = &blocks.blocks[..] else {
        panic!("one block is copied, not {blocks:?}");
    };
    assert_eq!((block.id, block.block_type), (notes, kind));
    assert_eq!(block.title, "Notes");
    assert!(
        fixture
            .test
            .take_requests()
            .iter()
            .all(|call| !matches!(call, HostCall::CutBlocks { blocks, .. } if !blocks.is_empty())),
        "a copy records no cut"
    );

    fixture.choose(notes, "Cut");
    assert_eq!(fixture.test.take_copied(formats::BLOCKS).len(), 1);
    let cuts = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::CutBlocks { blocks, holder, .. } if !blocks.is_empty() => {
                Some((blocks, holder))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cuts,
        [(vec![notes.into_bytes()], MY_FILES.into_bytes())],
        "a cut tells the host where the block was cut from"
    );
}
