use be_sdk_beui::be_block::BlockContent;
use be_type_database::database_view::DatabaseViewContent;

use super::*;

#[test]
fn a_new_view_of_a_database_in_no_folder_goes_to_unfiled() {
    let (mut harness, _editor) = editor();
    harness.click("database.new-view");
    harness.run();

    let store = harness.store();
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
        "no folder holds the database to put the view beside it, so it goes to Unfiled"
    );
}
