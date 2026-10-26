use block_editor_beui::be_block::metadata::{ICON, ICON_COLOR};
use block_editor_beui::beui::icons::ICON_FILE_TEXT;
use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildOperations, ShellCommand,
};

use super::*;

#[test]
fn icon_and_color_asks_the_host_from_the_row_and_the_row_shows_what_is_chosen() {
    let mut fixture = editor();
    let kind = Uuid::from_u128(1);
    let block = Uuid::from_u128(5);
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: kind.into_bytes(),
            display_name: "Note".to_owned(),
            color: Some([65, 145, 210]),
            icon_codepoint: ICON_FILE_TEXT.as_str().to_owned(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        ..Catalog::default()
    });
    let mut listed = BlockInfo::new(block, kind, BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    fixture.test.store().add_block(files_workspace(
        &fixture.host,
        std::slice::from_ref(&listed),
    ));
    fixture.test.store().add_block(listed.clone());
    fixture.settle();
    fixture.test.snapshot("block_icons_inherit_the_type_color");

    fixture.choose(block, "Icon and color…");
    let asked: Vec<_> = fixture
        .test
        .take_shell_commands()
        .into_iter()
        .filter_map(|(id, command)| match command {
            ShellCommand::Rename { anchor } => Some((id, anchor)),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked.len(),
        1,
        "choosing Icon and color asks the host for its rename card once"
    );
    assert_eq!(asked[0].0, block, "for the row's block");
    assert!(
        asked[0].1.is_some_and(|anchor| anchor.width() > 0.0),
        "from the row, so the card points at it"
    );

    listed.metadata.insert(&ICON, &"star".to_owned());
    listed.metadata.insert(&ICON_COLOR, &[203, 91, 125]);
    fixture.test.store().add_block(listed);
    fixture.settle();
    fixture.test.snapshot("a_block_has_a_custom_icon_and_color");
}
