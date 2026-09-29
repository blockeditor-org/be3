use block_editor_beui::be_block::{BlockContent, ImageContent, TextContent};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, FileSave, HostReply, HostRequest};

use super::*;

#[test]
fn exporting_a_text_block_saves_it_as_a_markdown_file() {
    let mut fixture = editor();
    let notes = Uuid::from_u128(2);
    let photo = Uuid::from_u128(3);
    let mut text = BlockInfo::new(notes, TextContent::CONTENT_TYPE, BlockParent::Root);
    text.name = Some("Trip notes".to_owned());
    text.named_by_hand = true;
    let mut image = BlockInfo::new(photo, ImageContent::CONTENT_TYPE, BlockParent::Root);
    image.name = Some("Beach".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![text, image]);
    fixture
        .test
        .hold(Some(notes), TextContent::new("# Day one\nSand."));
    fixture.test.hold(
        Some(photo),
        ImageContent::from_file("beach.jpg", vec![0xff, 0xd8, 0xff]),
    );
    fixture.settle();
    fixture.test.take_requests();

    fixture.choose(notes, "Export");

    let saves: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|(id, request)| match request {
            HostRequest::SaveFile(file) => Some((id, file)),
            _ => None,
        })
        .collect();
    let [(request, file)] = saves.as_slice() else {
        panic!("exporting asks the host to save one file, asked {saves:?}");
    };
    assert_eq!(file.name, "Trip notes.md");
    assert_eq!(file.mime_type, "text/markdown");
    assert_eq!(file.data, b"# Day one\nSand.");

    fixture.test.reply(
        *request,
        HostReply::FileSaved(FileSave::Failed("The disk is full".to_owned())),
    );
    fixture.settle();
    assert!(
        fixture.says("The disk is full"),
        "a save that failed says why"
    );

    fixture.choose(photo, "Export");
    let file = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(_, request)| match request {
            HostRequest::SaveFile(file) => Some(file),
            _ => None,
        })
        .expect("exporting an image asks the host to save it");
    assert_eq!(
        file.name, "beach.jpg",
        "an image keeps the file it came from"
    );
    assert_eq!(file.data, vec![0xff, 0xd8, 0xff]);
}
