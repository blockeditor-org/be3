use be_sdk_beui::be_block_content::BlockContent;

use super::*;

#[test]
fn a_new_view_beside_a_view_in_no_folder_goes_to_unfiled() {
    let mut fixture = editor(&[("Title", DatabaseFieldType::String)]);
    fixture.harness.click("database-view.views.new");
    fixture.settle();

    let store = fixture.harness.store();
    let views: Vec<_> = store
        .created()
        .into_iter()
        .filter_map(|id| store.block(id))
        .filter(|info| info.block_type == DatabaseViewContent::CONTENT_TYPE)
        .collect();
    let [view] = views.as_slice() else {
        panic!("one view is made: {views:?}");
    };
    assert_eq!(
        view.parent,
        Some(be_sdk_beui::BlockParent::Unfiled),
        "no folder holds the view to put the new one beside it, so it goes to Unfiled"
    );
}
