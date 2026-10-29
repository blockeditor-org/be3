use super::*;
use crate::unstyled::state::{DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane, DockSpecWindow};
use beui_view::types::Direction;

mod a_bar_keeps_its_placement_wherever_its_tab_goes;
mod a_bare_pane_shows_its_one_tab_and_takes_no_other;
mod a_fullscreen_tab_closed_and_opened_again_comes_back_in_its_pane;
mod a_group_cannot_be_dropped_inside_itself;
mod a_group_left_with_one_tab_becomes_that_tab_again;
mod a_groups_subtree_grafted_back_keeps_its_panes_and_their_keys;
mod a_kept_pane_added_in_the_code_splits_off_its_sibling;
mod a_locked_pane_inside_a_pinned_group_keeps_its_tabs_in_its_area;
mod a_locked_tab_cannot_float_into_a_window;
mod a_locked_tab_dropped_outside_its_area_stays_put;
mod a_locked_tab_found_outside_its_area_is_moved_back_home;
mod a_locked_tab_split_off_inside_its_area_takes_the_area_with_it;
mod a_pane_holding_only_a_group_takes_the_tabs_of_the_group;
mod a_pane_with_an_empty_view_stays_when_its_last_tab_leaves;
mod a_pinned_group_carries_its_pinned_tabs_wherever_it_goes;
mod a_pinned_group_in_the_code_keeps_its_tabs_home;
mod a_pinned_group_keeps_its_last_tab_and_survives_being_emptied;
mod a_pinned_group_only_takes_the_tabs_that_live_in_it;
mod a_pinned_tab_stays_in_its_group_until_it_is_unpinned;
mod a_plain_layout_read_from_a_state_rebuilds_the_same_state;
mod a_see_through_tab_stays_on_the_main_surface;
mod a_spec_seeds_the_layout_it_describes;
mod a_split_divides_its_area_between_the_panes_and_the_handle;
mod a_split_follows_the_code_until_the_user_moves_it;
mod a_split_leaves_each_pane_room_for_its_tab_bar;
mod a_tab_closed_out_of_the_way_hands_its_visits_to_the_tab_it_was_reached_from;
mod a_tab_closed_while_switching_leaves_the_switch;
mod a_tab_declared_in_a_window_that_is_not_open_opens_the_window;
mod a_tab_declared_later_in_a_locked_pane_is_locked_to_its_area;
mod a_tab_declared_later_joins_its_pane_after_its_declared_neighbour;
mod a_tab_dropped_on_another_tab_bar_lands_at_the_place_it_was_dropped;
mod a_tab_hosting_a_dock_becomes_a_pinned_group_in_its_place;
mod a_tab_whose_pane_is_gone_joins_the_nearest_open_tab;
mod a_tab_with_no_open_neighbour_floats_in_a_window;
mod a_tree_read_from_one_state_rebuilds_the_same_layout_in_another;
mod an_undeclared_tab_is_closed_and_the_one_shown_before_it_takes_its_place;
mod an_unlocked_tab_goes_anywhere_and_locks_again_once_home;
mod cancelling_a_switch_leaves_the_tab_that_was_shown;
mod closing_the_last_tab_of_a_window_closes_the_window;
mod dropping_a_tab_onto_another_tab_groups_them;
mod emptying_a_pane_gives_its_room_to_the_pane_it_was_split_from;
mod going_back_closes_a_tab_it_opened_and_returns_from_a_tab_switched_to;
mod setting_a_tree_moves_its_tabs_in_and_keeps_the_focused_tab_focused;
mod showing_a_tab_inside_a_group_selects_the_group;
mod splitting_a_tab_with_the_next_makes_a_group_holding_a_split_view;
mod splitting_off_a_pane_keeps_its_tabs_and_its_sidebar;
mod switching_walks_the_tabs_from_the_one_shown_last;
mod the_recent_tabs_begin_with_the_one_shown_last;
mod two_tabs_unpinned_from_a_group_can_be_grouped_outside_it;
mod ungrouping_puts_the_tabs_back_where_the_group_was;

fn pane_spec(key: &str) -> DockSpecPane {
    DockSpecPane {
        key: key.to_owned(),
        entries: Vec::new(),
        active: None,
        vertical: false,
        icons_only: false,
        sidebar: SIDEBAR_WIDTH,
        keep: false,
        locked: false,
        bare: false,
    }
}

fn pane(key: &str, tabs: &[u64]) -> DockSpecNode {
    DockSpecNode::Pane(DockSpecPane {
        entries: tabs
            .iter()
            .map(|tab| DockSpecEntry::Tab(TabId::new(*tab)))
            .collect(),
        ..pane_spec(key)
    })
}

fn kept(key: &str, tabs: &[u64]) -> DockSpecNode {
    let mut node = pane(key, tabs);
    if let DockSpecNode::Pane(pane) = &mut node {
        pane.keep = true;
    }
    node
}

fn split(
    key: &str,
    direction: Direction,
    fraction: f32,
    first: DockSpecNode,
    second: DockSpecNode,
) -> DockSpecNode {
    DockSpecNode::Split {
        key: key.to_owned(),
        direction,
        fraction,
        first: Box::new(first),
        second: Box::new(second),
    }
}

fn spec(main: DockSpecNode) -> DockSpec {
    DockSpec {
        main: Some(main),
        ..DockSpec::default()
    }
}

fn locked(key: &str, tabs: &[u64]) -> DockSpecNode {
    let mut node = pane(key, tabs);
    if let DockSpecNode::Pane(pane) = &mut node {
        pane.locked = true;
    }
    node
}

fn sidebar_and_editors(files: &[u64], editors: &[u64]) -> DockSpec {
    spec(split(
        "workspace",
        Direction::Horizontal,
        0.25,
        locked("files", files),
        locked("editors", editors),
    ))
}

fn leaf_of(state: &DockState, tab: u64) -> LeafId {
    state.find(TabId::new(tab)).expect("the tab is open").leaf
}
