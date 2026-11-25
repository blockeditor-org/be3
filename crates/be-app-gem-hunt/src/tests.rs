use be_sdk_beui::{BlockCommand, ChannelEvent, JoinedMember, MemberEvent, MemberSaid};
use be_sdk_test::BeuiTest;
use be_sdk_test::prelude::*;
use be_type_gem_hunt::{Hunt, HuntContent};
use uuid::Uuid;

use crate::root::{HuntHost, HuntRoot};
use crate::wire::{Ask, Board, decode, encode};

mod a_player_sees_only_what_the_host_tells_it_and_digs_by_asking;
mod the_host_digs_where_a_player_asks_and_tells_each_only_what_is_dug;
mod the_world_shows_one_who_may_read_it_where_every_gem_lies;

/// A world with gems at the top left corner and in the middle of the second row, and two
/// cells dug: the corner's gem, by `by`, and the cell below the corner.
fn world(by: Uuid) -> HuntContent {
    let mut hunt = Hunt {
        gems: vec![0, 9, 20, 27, 31, 35],
        ..Hunt::default()
    };
    hunt.dug = [(0, by), (6, by)].into_iter().collect();
    HuntContent::new(&hunt)
}

/// What the editor or component says on the host channel, taken.
fn told<A: be_sdk_beui::BeuiRoot + be_sdk_beui::AppEditor>(
    test: &mut BeuiTest<A>,
) -> Vec<BlockCommand> {
    test.take_block_commands()
        .into_iter()
        .map(|(_, command)| command)
        .filter(|command| {
            matches!(
                command,
                BlockCommand::TellHost(_) | BlockCommand::TellMember(_)
            )
        })
        .collect()
}

fn shown<A: be_sdk_beui::BeuiRoot + be_sdk_beui::AppEditor>(
    test: &BeuiTest<A>,
    test_id: &str,
) -> String {
    let node = test
        .document()
        .find_test_id(test_id)
        .unwrap_or_else(|| panic!("nothing shows {test_id}"));
    test.document()
        .node_detail(node)
        .unwrap_or_else(|| panic!("{test_id} shows no text"))
}
