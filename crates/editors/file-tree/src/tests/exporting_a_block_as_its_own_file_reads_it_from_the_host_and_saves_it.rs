use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, FileSave, Press, SavedFile,
};
use block_ui_test::HostCall;

use super::*;

#[test]
fn exporting_a_block_as_its_own_file_reads_it_from_the_host_and_saves_it() {
    let mut fixture = editor();
    let text = Uuid::from_u128(0x7465_7874);
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: text.into_bytes(),
            display_name: "Text".to_owned(),
            color: None,
            icon_codepoint: String::new(),
            children: block_editor_beui::ChildOperations::default(),
            press: Press::Open,
            plain_file: true,
        }],
        templates: Vec::new(),
        editors: Vec::new(),
    });
    let notes = Uuid::from_u128(2);
    let mut info = BlockInfo::new(notes, text, BlockParent::Block(MY_FILES));
    info.set_name("Trip notes".to_owned());
    my_files(&mut fixture, vec![info]);
    fixture.settle();
    fixture.test.take_requests();

    fixture.choose(notes, "Export as");
    fixture.click_item("Text file");
    let reads: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::ReadPlainFile { reply, block } => Some((reply, Uuid::from_bytes(block))),
            _ => None,
        })
        .collect();
    let [(request, block)]: [_; 1] = reads.try_into().unwrap_or_else(|reads: Vec<_>| {
        panic!("exporting asks the host for the block's own file, asked {reads:?}")
    });
    assert_eq!(block, notes);
    let file = SavedFile {
        name: "Trip notes.md".to_owned(),
        mime_type: "text/markdown".to_owned(),
        data: b"# Day one\nSand.".to_vec(),
    };
    fixture.test.answer(request, Ok(file.clone()));
    fixture.settle();

    let saves: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::SaveFile { reply, file } => Some((reply, file)),
            _ => None,
        })
        .collect();
    let [(request, saved)]: [_; 1] = saves
        .try_into()
        .unwrap_or_else(|saves: Vec<_>| panic!("the file the host read is saved, asked {saves:?}"));
    assert_eq!(saved, file);
    fixture
        .test
        .answer(request, FileSave::Failed("The disk is full".to_owned()));
    fixture.settle();
    assert!(
        fixture.says("The disk is full"),
        "a save that failed says why"
    );

    fixture.choose(notes, "Export as");
    fixture.click_item("Text file");
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ReadPlainFile { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("exporting again reads the file again");
    fixture
        .test
        .answer(
            request,
            Err(block_editor_beui::Failure::Failed(
                "The block could not be read".to_owned(),
            )),
        );
    fixture.settle();
    assert!(
        fixture.says("The block could not be read"),
        "a read that failed says why"
    );
}
