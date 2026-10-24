use block_editor_beui::be_block::{BlockContent, FolderContent};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, FileSave, HostReply, HostRequest};

use super::*;

#[test]
fn exporting_any_block_as_a_block_file_asks_the_host_for_it() {
    let mut fixture = editor();
    folders_zoom(&mut fixture);
    let folder = Uuid::from_u128(2);
    let mut info = BlockInfo::new(folder, FolderContent::CONTENT_TYPE, BlockParent::Root);
    info.set_name("Trips".to_owned());
    fixture.host.set_blocks(BlockQuery::Roots, vec![info]);
    fixture.settle();
    fixture.test.take_requests();

    fixture.choose(folder, "Export as");
    fixture
        .test
        .snapshot("exporting_any_block_as_a_block_file_asks_the_host_for_it");
    let document = fixture.test.document();
    assert!(
        document
            .root()
            .and_then(|root| open_item(document, root, "Text file", false))
            .is_none(),
        "a folder has no format of its own to export as"
    );
    fixture.click_item("Block file");

    let exports: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|(id, request)| match request {
            HostRequest::ExportBlock(block) => Some((id, Uuid::from_bytes(block))),
            _ => None,
        })
        .collect();
    let [(request, block)] = exports.as_slice() else {
        panic!("exporting a block file asks the host to export one block, asked {exports:?}");
    };
    assert_eq!(*block, folder);

    fixture.test.reply(
        *request,
        HostReply::FileSaved(FileSave::Failed("The disk is full".to_owned())),
    );
    fixture.settle();
    assert!(
        fixture.says("The disk is full"),
        "an export that failed says why"
    );
}
