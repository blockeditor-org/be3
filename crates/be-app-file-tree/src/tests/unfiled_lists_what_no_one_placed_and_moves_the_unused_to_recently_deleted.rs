use be_sdk_beui::be_block::CREATED_BY;
use be_sdk_beui::beui::Input;
use be_sdk_beui::{BlockCommand, BlockInfo, BlockParent, BlockQuery};

use super::*;

fn unfiled(id: u128, name: &str, made_by: Option<&str>) -> BlockInfo {
    let mut block = BlockInfo::new(Uuid::from_u128(id), PLAIN, BlockParent::Unfiled);
    block.set_name(name);
    if let Some(app) = made_by {
        block.metadata.insert(&CREATED_BY, &app.to_owned());
    }
    block
}

#[test]
fn unfiled_lists_what_no_one_placed_and_moves_the_unused_to_recently_deleted() {
    use be_sdk_beui::be_block::BlockContent;
    let mut fixture = editor();
    let dev_vms = named_block(0x10, FolderContent::CONTENT_TYPE, MY_FILES, "Dev VMs");
    my_files(&mut fixture, vec![dev_vms.clone()]);
    fixture.settle();
    let shell = unfiled(0x20, "Shell", Some("boat.dev"));
    let grid = unfiled(0x30, "Compiled grid", Some("be3.logic-grid"));
    let note = unfiled(0x40, "Note", None);
    fixture.host.set_blocks(
        BlockQuery::Unfiled,
        vec![shell.clone(), grid.clone(), note.clone()],
    );
    fixture
        .host
        .set_blocks(BlockQuery::Backrefs(shell.id), vec![dev_vms.clone()]);
    fixture
        .host
        .set_blocks(BlockQuery::Backrefs(grid.id), Vec::new());
    fixture
        .host
        .set_blocks(BlockQuery::Backrefs(note.id), Vec::new());
    assert!(
        fixture.test.shown("file-tree.unfiled.row"),
        "Unfiled is a section of the files pane"
    );
    fixture.test.click("file-tree.unfiled.chevron");
    fixture.settle();
    fixture.settle();

    let top = |fixture: &Fixture, id: Uuid| fixture.test.rect_of(&format!("file-tree.{id}.row")).top();
    assert!(
        top(&fixture, note.id) < top(&fixture, grid.id) && top(&fixture, grid.id) < top(&fixture, shell.id),
        "newest first"
    );
    assert!(
        fixture.test.rect_of(&format!("file-tree.{MY_FILES}.row")).top()
            < fixture.test.rect_of("file-tree.unfiled.row").top(),
        "below My files"
    );
    let document = fixture.test.document();
    let root = document.root().expect("the files pane has a root");
    assert!(says_within(&document, root, "boat.dev · used in Dev VMs"));
    assert!(says_within(&document, root, "be3.logic-grid · not used anywhere"));
    assert!(says_within(&document, root, "Not used anywhere"));
    drop(document);
    fixture.test.settle();
    fixture
        .test
        .snapshot("unfiled_says_which_app_made_each_block_and_where_it_is_used");

    let pos = fixture.test.rect_of("file-tree.unfiled.row").center();
    fixture.test.step(vec![Input::PointerMoved(pos)]);
    fixture.test.step(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    fixture.test.step(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: false,
    }]);
    fixture.settle();
    fixture.click_item("Move unused to Recently Deleted");

    let mut deleted: Vec<Uuid> = fixture
        .test
        .take_block_commands()
        .into_iter()
        .filter_map(|(id, command)| match command {
            BlockCommand::Delete { source, .. } => {
                assert_eq!(
                    BlockParent::decode(source),
                    BlockParent::Unfiled,
                    "deleted from Unfiled, so Restore puts it back there"
                );
                Some(id)
            }
            _ => None,
        })
        .collect();
    deleted.sort();
    assert_eq!(
        deleted,
        vec![grid.id, note.id],
        "only what nothing uses goes, and the rest stays"
    );
}
