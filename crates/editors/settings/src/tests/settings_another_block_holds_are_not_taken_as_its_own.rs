use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{SETTINGS, WORKSPACE_TYPE};
use block_editor_beui::BlockTypeDescriptor;

use super::*;

const HOTBAR: Uuid = Uuid::from_u128(0x6c6f_6769_632d_686f_7462_6172_0101_0101);
const TAGS: Uuid = Uuid::from_u128(0x7461_6773);
const FOLDER: Uuid = Uuid::from_u128(0xf01d);

fn described(block_type: Uuid, name: &str) -> BlockTypeDescriptor {
    BlockTypeDescriptor {
        block_type: block_type.into_bytes(),
        display_name: name.to_owned(),
        color: None,
        icon_codepoint: String::new(),
        children: Default::default(),
        press: Default::default(),
        plain_file: false,
    }
}

#[test]
fn settings_another_block_holds_are_not_taken_as_its_own() {
    let [
        workspace,
        folder,
        elsewhere,
        decoy,
        decoy_tags,
        stray_hotbar,
        folder_settings,
    ] = std::array::from_fn(|index| Uuid::from_u128(0xd0_0001 + index as u128));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0xd0_0100));
    host.set_block_type(SettingsContent::CONTENT_TYPE);
    let mut editor = BeuiTest::<SettingsApp>::new(OwnedEditor::new(host, folder_settings));
    editor.set_size(Vec2::new(720.0, 420.0));
    editor.block_types(vec![described(HOTBAR, "Hotbar"), described(TAGS, "Tags")]);
    let store = editor.store();
    let mut team = BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root);
    team.set_name("Team");
    team.metadata.insert(&SETTINGS, &BlockRef(decoy));
    store.add_block(team);
    let mut designs = BlockInfo::new(folder, FOLDER, BlockParent::Block(workspace));
    designs.set_name("Designs");
    designs.metadata.insert(&SETTINGS, &BlockRef(folder_settings));
    store.add_block(designs);
    for (id, block_type, parent) in [
        (elsewhere, FOLDER, BlockParent::Root),
        (decoy, SettingsContent::CONTENT_TYPE, BlockParent::Block(elsewhere)),
        (decoy_tags, TAGS, BlockParent::Block(decoy)),
        (stray_hotbar, HOTBAR, BlockParent::Block(elsewhere)),
        (
            folder_settings,
            SettingsContent::CONTENT_TYPE,
            BlockParent::Block(folder),
        ),
    ] {
        store.add_block(BlockInfo::new(id, block_type, parent));
    }
    let mut decoyed = SettingsContent::default();
    decoyed.apply(&Settings::set_kind(TAGS, decoy_tags));
    decoyed.apply(&Settings::set_kind(HOTBAR, stray_hotbar));
    store.hold(Some(decoy), decoyed);
    editor.hold(None, SettingsContent::default());
    editor.run();
    for kind in [TAGS, HOTBAR] {
        assert!(
            editor
                .document()
                .find_test_id(&format!("settings.above.{kind}.open"))
                .is_none(),
            "a block's metadata named settings another block holds"
        );
    }
}
