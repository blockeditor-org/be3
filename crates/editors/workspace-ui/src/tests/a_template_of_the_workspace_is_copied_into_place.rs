use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{
    BlockContent, Home, HomeContent, SETTINGS, Settings, SettingsContent, WORKSPACE_TYPE,
};
use block_editor_beui::{BlockInfo, BlockLocation, ImportedBlock, PickedBlock};
use block_ui_test::HostCall;

use super::*;

#[test]
fn a_template_of_the_workspace_is_copied_into_place() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let [workspace, settings, home, template] =
        [1, 2, 3, 4].map(|id| Uuid::from_u128(0x7e_0000 + id));
    let store = fixture.test.store();
    let mut team = BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root);
    team.metadata.insert(&SETTINGS, &BlockRef(settings));
    team.references = vec![settings, template];
    store.add_block(team);
    store.add_block(BlockInfo::new(
        settings,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Block(workspace),
    ));
    store.add_block(BlockInfo::new(
        home,
        HomeContent::CONTENT_TYPE,
        BlockParent::Block(settings),
    ));
    let mut weekly = BlockInfo::new(template, SHOWN_TYPE, BlockParent::Block(workspace));
    weekly.set_name("Weekly plan");
    store.add_block(weekly);
    let mut kinds = SettingsContent::default();
    kinds.apply(&Settings::set_kind(HomeContent::CONTENT_TYPE, home));
    fixture.test.hold(Some(settings), kinds);
    let mut kept = HomeContent::default();
    kept.apply(&Home::keep_template(template, true));
    fixture.test.hold(Some(home), kept);
    fixture.settle();

    let mut fixture = picking(
        &fixture,
        filter(),
        BlockLocation::Block(workspace.into_bytes()),
    );
    fixture.settle();
    fixture.test.take_requests();
    fixture.test.click("picker.templates");
    fixture.settle();
    fixture
        .test
        .snapshot("a_template_of_the_workspace_is_copied_into_place");
    fixture.test.click(format!("picker.template.{template}"));
    fixture.settle();

    let copies: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::CopyBlocks {
                reply,
                source,
                parent,
            } => Some((reply, Uuid::from_bytes(source), parent)),
            _ => None,
        })
        .collect();
    let [(request, source, parent)]: [_; 1] = copies.try_into().unwrap_or_else(|copies: Vec<_>| {
        panic!("choosing a template asks the host to copy it, asked {copies:?}")
    });
    assert_eq!(source, template);
    assert_eq!(parent, BlockLocation::Block(workspace.into_bytes()));
    let copy = Uuid::from_u128(0x7e_0005);
    fixture.test.answer(
        request,
        Ok(Some(ImportedBlock {
            block_id: copy.into_bytes(),
            block_type: SHOWN_TYPE.into_bytes(),
        })),
    );
    fixture.settle();
    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(Some(PickedBlock {
            block_id: copy.into_bytes(),
            block_type: SHOWN_TYPE.into_bytes(),
            linked: false,
            placed: false,
        }))],
        "the copy answers the pick"
    );
}
