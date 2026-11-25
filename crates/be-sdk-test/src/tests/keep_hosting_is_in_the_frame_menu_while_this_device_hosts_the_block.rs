use be_sdk_api::MenuCheck;
use be_sdk_api::blocks::HostingHere;
use be_sdk_beui::beui::styled::Root;
use beui::Vec2;

use super::*;

struct HostedRoot;

#[component]
impl BeuiRoot for HostedRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <Text>"A world"</Text>
            </Root>
        }
    }
}

/// The frame menu, and the palette, offer Keep hosting only while this device hosts the block
/// (guides/blocks/hosted.md, "Hosting"), ticked once it keeps hosting it, and picking it asks the
/// host to keep hosting, or to stop keeping it.
#[test]
fn keep_hosting_is_in_the_frame_menu_while_this_device_hosts_the_block() {
    let block = Uuid::new_v4();
    let mut test = BeuiTest::<HostedRoot>::editing(block)
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    let offered =
        |test: &BeuiTest<HostedRoot>| test.menu_ids().iter().any(|id| id == "editor.keep-hosting");
    let in_palette = |test: &BeuiTest<HostedRoot>| {
        test.described_actions()
            .iter()
            .any(|action| action.id == "editor.keep-hosting")
    };
    assert!(!offered(&test), "not offered while another device hosts it");
    assert!(!in_palette(&test), "nor in the palette");

    test.set_hosting(HostingHere::Yes);
    assert!(offered(&test), "offered once this device hosts it");
    assert!(in_palette(&test), "and in the palette");
    assert_eq!(
        test.menu_entry("editor.keep-hosting").check,
        MenuCheck::Checkbox(false)
    );
    test.pick_menu("editor.keep-hosting");
    assert!(
        test.take_block_commands()
            .contains(&(block, be_sdk_beui::BlockCommand::KeepHosting(true))),
        "picking it asks the host to keep hosting"
    );

    test.set_hosting(HostingHere::Kept);
    assert_eq!(
        test.menu_entry("editor.keep-hosting").check,
        MenuCheck::Checkbox(true),
        "ticked while kept"
    );
    test.pick_menu("editor.keep-hosting");
    assert!(
        test.take_block_commands()
            .contains(&(block, be_sdk_beui::BlockCommand::KeepHosting(false))),
        "picking it again stops keeping it"
    );

    test.set_hosting(HostingHere::No);
    assert!(!offered(&test), "gone once this device no longer hosts it");
    assert!(!in_palette(&test), "and from the palette");
}

test_editor!(HostedRoot);
