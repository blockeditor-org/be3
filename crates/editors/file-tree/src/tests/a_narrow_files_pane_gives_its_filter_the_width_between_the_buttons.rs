use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn a_narrow_files_pane_gives_its_filter_the_width_between_the_buttons() {
    for (width, zoomed) in [(242.0, false), (242.0, true), (800.0, false)] {
        let mut fixture = editor_sized(Some(Vec2::new(width, 600.0)));
        containers_hold_blocks(&mut fixture);
        my_files(&mut fixture, Vec::new());
        fixture.settle();
        if zoomed {
            let team = Uuid::from_u128(4);
            let mut held = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
            held.set_name("A workspace with a long name");
            held.author = Some(fixture.host.account_id());
            let own = files_workspace(&fixture.host, &[]);
            fixture.host.set_blocks(BlockQuery::Roots, vec![own, held]);
            fixture.settle();
            fixture.test.click(format!("file-tree.{team}.row"));
            fixture.settle();
            fixture
                .host
                .set_blocks(BlockQuery::References(team), Vec::new());
            fixture.settle();
            assert!(fixture.test.shown("file-tree.back"));
        }

        let filter = fixture.test.rect_of("file-tree.filter");
        let new = fixture.test.rect_of("file-tree.new");
        let account = fixture.test.rect_of("file-tree.app-menu");
        assert!(
            account.max.x <= width && width - account.max.x < 24.0,
            "at {width} wide (zoomed: {zoomed}) the account button sits at the right edge, not packed left: {account:?}"
        );
        assert!(
            filter.max.x <= new.min.x && new.max.x <= account.min.x,
            "the filter, New and the account button are in that order: {filter:?} {new:?} {account:?}"
        );
        assert!(
            new.min.x - filter.max.x < 16.0,
            "the filter fills up to New: {filter:?} {new:?}"
        );
    }
}
