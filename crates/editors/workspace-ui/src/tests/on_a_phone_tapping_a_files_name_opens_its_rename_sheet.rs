use block_editor_beui::{
    ChildStatus, EditorCapabilities, EditorInstanceId, EditorRegion, InteractionMode, MenuCommand,
    MenuEntry, MenuPin, ResizeMode, ShellDialog,
};

use super::*;

#[test]
fn on_a_phone_tapping_a_files_name_opens_its_rename_sheet() {
    let (mut fixture, notes) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    fixture.test.catalog(catalog());
    let mut info = block_editor_beui::BlockInfo::new(notes, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Notes");
    fixture.test.store().add_block(info);
    show(&mut fixture, notes, None);
    let child = fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content.block_id() == Some(notes.into_bytes()))
        .expect("the block is on show")
        .child;
    fixture.test.report_children(|placement| ChildStatus {
        instance: EditorInstanceId(0),
        region: EditorRegion::Frame,
        child: placement.child,
        available: true,
        intrinsic: None,
        aspect_ratio: None,
        active: false,
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::None,
        error: None,
        menu: match placement.child == child {
            true => MenuEntry::flatten(&[MenuEntry::Command(MenuCommand {
                id: "editor.rename".into(),
                label: "Rename".into(),
                enabled: true,
                pin: MenuPin::Title,
                ..MenuCommand::default()
            })]),
            false => Vec::new(),
        },
        creation: None,
        preview: Default::default(),
        dialog: Default::default(),
        picked: None,
        card: false,
    });
    fixture.settle();
    fixture.test.settle();
    fixture
        .test
        .snapshot("a_phones_bar_offers_its_files_name_to_rename");

    fixture.test.click("dock.title");
    fixture.settle();
    assert_eq!(
        fixture.test.take_child_menu_picks(),
        [(child, "editor.rename".to_owned())],
        "tapping the name runs the block's rename"
    );

    fixture.test.show_dialog(
        notes,
        ShellDialog::Rename {
            anchor: None,
            within: Some(notes.into_bytes()),
        },
    );
    fixture.settle();
    fixture.test.settle();
    assert!(
        fixture.test.shown("rename.name") && fixture.test.shown("rename.color.pink"),
        "the name, color and icon are chosen in one sheet"
    );
    fixture.test.snapshot("a_phone_renames_a_file_in_a_sheet");
}
