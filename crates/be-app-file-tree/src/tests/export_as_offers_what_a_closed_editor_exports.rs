use be_sdk_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, EditorDescriptor, ExporterDescriptor,
    Press,
};

use super::*;

#[test]
fn export_as_offers_what_a_closed_editor_exports() {
    let mut fixture = editor();
    let canvas_type = Uuid::from_u128(0x6361_6e76_6173);
    let exporter = |name: &str, label: &str| ExporterDescriptor {
        name: name.to_owned(),
        label: label.to_owned(),
    };
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: canvas_type.into_bytes(),
            display_name: "Canvas".to_owned(),
            color: None,
            icon_codepoint: String::new(),
            children: be_sdk_beui::ChildOperations::default(),
            press: Press::Open,
            plain_file: false,
        }],
        templates: Vec::new(),
        editors: vec![EditorDescriptor {
            editor: Uuid::from_u128(0xc0de).into_bytes(),
            edits: be_sdk_beui::Edits::Type(canvas_type.into_bytes()),
            name: "Canvas".to_owned(),
            icon_codepoint: String::new(),
            extensions: Vec::new(),
            exporters: vec![exporter("png", "PNG image"), exporter("svg", "SVG image")],
        }],
    });
    let board = Uuid::from_u128(2);
    let mut info = BlockInfo::new(board, canvas_type, BlockParent::Block(MY_FILES));
    info.set_name("Board".to_owned());
    my_files(&mut fixture, vec![info]);
    fixture.settle();

    fixture.choose(board, "Export as");
    assert!(
        fixture.offers("PNG image") && fixture.offers("SVG image"),
        "Export as lists what the canvas's editor exports as, though it is not open"
    );
    fixture.click_item("SVG image");
    let asked = fixture.test.take_export_requests();
    let [(block, request)] = asked.as_slice() else {
        panic!("exporting asks the host for one Export dialog, asked {asked:?}");
    };
    assert_eq!(*block, board);
    assert_eq!(request.exporter, "svg");
    assert!(
        request.settings.is_empty(),
        "an export from the file tree starts from the exporter's own settings"
    );
}
