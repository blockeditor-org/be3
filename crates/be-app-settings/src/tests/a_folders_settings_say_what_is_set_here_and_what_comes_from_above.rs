use be_sdk_beui::be_block::{ANYONE_INSIDE, WORKSPACE_TYPE};
use be_sdk_beui::be_block_content::SETTINGS;
use be_sdk_beui::be_block_content::be_block_model::BlockRef;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::native::Document;
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, BlockTypeDescriptor, Failure};
use be_sdk_test::HostCall;

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

fn named(id: Uuid, block_type: Uuid, parent: BlockParent, name: &str, settings: Uuid) -> BlockInfo {
    let mut info = BlockInfo::new(id, block_type, parent);
    info.set_name(name);
    info.metadata.insert(&SETTINGS, &BlockRef(settings));
    info
}

#[test]
fn a_folders_settings_say_what_is_set_here_and_what_comes_from_above() {
    let [
        workspace,
        folder,
        workspace_settings,
        folder_settings,
        tags,
        workspace_hotbar,
        hotbar,
    ] = std::array::from_fn(|index| Uuid::from_u128(0xf0_0001 + index as u128));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0xf0_0100));
    host.set_block_type(SettingsContent::CONTENT_TYPE);
    let mut editor = BeuiTest::<SettingsRoot>::new(OwnedEditor::new(host, folder_settings));
    editor.set_size(Vec2::new(720.0, 420.0));
    editor.block_types(vec![described(HOTBAR, "Hotbar"), described(TAGS, "Tags")]);
    let store = editor.store();
    store.add_block(named(
        workspace,
        WORKSPACE_TYPE,
        BlockParent::Root,
        "Team",
        workspace_settings,
    ));
    store.add_block(named(
        folder,
        FOLDER,
        BlockParent::Block(workspace),
        "Designs",
        folder_settings,
    ));
    for (id, block_type, parent) in [
        (workspace_settings, SettingsContent::CONTENT_TYPE, workspace),
        (folder_settings, SettingsContent::CONTENT_TYPE, folder),
        (tags, TAGS, workspace_settings),
        (workspace_hotbar, HOTBAR, workspace_settings),
        (hotbar, HOTBAR, folder_settings),
    ] {
        store.add_block(BlockInfo::new(id, block_type, BlockParent::Block(parent)));
    }
    let mut above = SettingsContent::default();
    above.apply(&Settings::set_kind(TAGS, tags));
    above.apply(&Settings::set_kind(HOTBAR, workspace_hotbar));
    store.hold(Some(workspace_settings), above);
    let mut here = SettingsContent::default();
    here.apply(&Settings::set_kind(HOTBAR, hotbar));
    editor.hold(None, here);
    editor.run();

    let asked = editor
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListAccess { reply, block } if Uuid::from_bytes(block) == hotbar => {
                Some(reply)
            }
            _ => None,
        })
        .expect("a manager is shown who can read the hotbar");
    editor.answer(
        asked,
        Ok(AccessListing {
            grants: vec![AccessGrant {
                account: ANYONE_INSIDE.into_bytes(),
                email: String::new(),
                display_name: "Anyone who can view a block inside".to_owned(),
                administrator: false,
                author: false,
                granted: Some(AccessLevel::View),
                inherited: AccessLevel::None,
                effective: AccessLevel::View,
                source: Some(hotbar.into_bytes()),
                waiting: false,
            }],
            company: false,
            set_inside: 0,
        }),
    );
    editor.run();
    assert!(
        editor
            .document()
            .find_test_id(&format!("settings.above.{TAGS}.open"))
            .is_some()
    );
    assert!(
        editor
            .document()
            .find_test_id(&format!("settings.above.{HOTBAR}.open"))
            .is_none(),
        "what is set here replaces what is set above"
    );
    editor.snapshot("a_folders_settings_say_what_is_set_here_and_what_comes_from_above");

    editor.click(&format!("settings.kind.{HOTBAR}.visibility"));
    editor.run();
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.run();
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
    let (reply, changed) = editor
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::SetAccess {
                reply,
                block,
                account,
                access,
            } => Some((
                reply,
                (Uuid::from_bytes(block), Uuid::from_bytes(account), access),
            )),
            _ => None,
        })
        .expect("choosing who can read the hotbar asks the host");
    assert_eq!(changed, (hotbar, ANYONE_INSIDE, AccessLevel::None));

    editor.answer(reply, Err(Failure::Failed(OFFLINE.to_owned())));
    editor.run();
    let refused = {
        let document = editor.document();
        document
            .root()
            .is_some_and(|root| says(&document, root, OFFLINE))
    };
    assert!(refused, "a choice the host refused says why on its row");
    editor.snapshot("a_folders_settings_say_why_a_choice_of_who_reads_it_did_not_take");
}

const OFFLINE: &str =
    "You're offline. Sharing changes need a connection; try again once you're back online.";

fn says(document: &Document, node: NodeId, words: &str) -> bool {
    document
        .arena
        .kind_of(node)
        .is_some_and(|kind| document.text(kind).contains(words))
        || document
            .children(node)
            .into_iter()
            .any(|child| says(document, child, words))
}
