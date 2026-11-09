use be_sdk_beui::be_block::be_block_model::BlockRef;
use be_sdk_beui::be_block::{
    BlockContent, Home, HomeContent, SETTINGS, Settings, SettingsContent, WORKSPACE_TYPE,
};
use be_sdk_beui::{BlockInfo, BlockLocation};
use be_sdk_test::HostCall;

use super::*;

#[test]
fn an_import_or_template_copy_with_no_place_is_made_in_unfiled() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let my_files = be_sdk_beui::be_block::my_files(
        fixture.host.organization_id(),
        fixture.host.account_id(),
    );
    let [settings, home, template] = [2, 3, 4].map(|id| Uuid::from_u128(0x7e_1000 + id));
    let store = fixture.test.store();
    let mut files = BlockInfo::new(my_files, WORKSPACE_TYPE, BlockParent::Root);
    files.metadata.insert(&SETTINGS, &BlockRef(settings));
    files.references = vec![settings, template];
    store.add_block(files);
    store.add_block(BlockInfo::new(
        settings,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Block(my_files),
    ));
    store.add_block(BlockInfo::new(
        home,
        HomeContent::CONTENT_TYPE,
        BlockParent::Block(settings),
    ));
    let mut weekly = BlockInfo::new(template, SHOWN_TYPE, BlockParent::Block(my_files));
    weekly.set_name("Weekly plan");
    store.add_block(weekly);
    let mut kinds = SettingsContent::default();
    kinds.apply(&Settings::set_kind(HomeContent::CONTENT_TYPE, home));
    fixture.test.hold(Some(settings), kinds);
    let mut kept = HomeContent::default();
    kept.apply(&Home::keep_template(template, true));
    fixture.test.hold(Some(home), kept);
    fixture.settle();

    let mut fixture = picking(&fixture, filter(), BlockLocation::Unfiled);
    fixture.settle();
    fixture.test.take_requests();
    fixture.test.click("picker.templates");
    fixture.settle();
    fixture.test.click(format!("picker.template.{template}"));
    fixture.settle();
    let copied = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::CopyBlocks { parent, .. } => Some(parent),
            _ => None,
        })
        .expect("choosing a template asks the host to copy it");
    assert_eq!(
        copied,
        BlockLocation::Unfiled,
        "a template copied with no place chosen goes to Unfiled"
    );
}

#[test]
fn an_import_with_no_place_is_made_in_unfiled() {
    let (fixture, _) = editor();
    let mut fixture = picking(&fixture, filter(), BlockLocation::Unfiled);
    fixture.settle();
    fixture.test.take_requests();

    fixture.test.click("picker.import");
    fixture.settle();
    let place = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ImportBlocks { parent, .. } => Some(parent),
            _ => None,
        })
        .expect("choosing a file asks the host to import one");
    assert_eq!(
        place,
        BlockLocation::Unfiled,
        "a file imported with no place chosen goes to Unfiled"
    );
}
