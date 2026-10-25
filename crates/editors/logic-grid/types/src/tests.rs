use be_block::{Edit, LiveEdit, Merge, Undo};
use be_commit::MergeResult;
use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, Point, Scale, Wire,
};

use uuid::Uuid;

use crate::{LogicGameContent, LogicGridContent, LogicGridOperation};

mod a_component_moved_on_both_sides_conflicts_and_keeps_ours;
mod a_component_removed_on_one_side_and_turned_on_the_other_stays_removed;
mod a_logic_grid_edit_keeps_its_wires_normalized_and_follows_its_children;
mod a_wire_cut_on_one_side_stays_cut_when_the_other_extends_it;
mod a_wire_cut_stays_cut_when_someone_extends_it_at_once;
mod a_wire_removed_on_one_side_stays_removed_when_the_other_branches_off_it;
mod compiled_logic_places_as_its_own_block_and_references_what_it_calls;
mod compiling_a_grid_derives_ports_from_its_inputs_and_outputs;
mod compiling_an_empty_grid_is_refused;
mod components_added_by_two_peers_at_once_are_both_kept;
mod components_added_on_both_sides_offline_are_both_kept;
mod logic_game_solutions_keep_their_order_per_challenge;
mod logic_grids_merge_a_move_and_a_turn_of_one_component;
mod solutions_and_quiz_answers_made_on_both_sides_merge_to_both;
mod the_same_solution_inserted_by_two_peers_at_once_is_listed_once;
mod the_same_wire_drawn_on_both_sides_merges_to_one;
mod undoing_a_component_removal_brings_it_back_with_its_wires_untouched;
mod undoing_a_wire_cut_joins_the_wire_again;
mod undoing_a_wire_keeps_the_part_someone_else_drew_onto_it;
mod unpinning_a_component_removes_it_from_every_folder;
mod wires_drawn_on_each_side_that_meet_join_into_one;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn merged<C: Merge>(base: &C, ours: &C, theirs: &C) -> (C, usize) {
    match C::merge3(base, ours, theirs) {
        MergeResult::Clean(value) => (value, 0),
        MergeResult::Conflicted { value, conflicts } => (value, conflicts),
    }
}

fn undone<C: Undo>(content: &mut C, operation: C::Op) -> C::Step {
    let step = content
        .step(&operation)
        .expect("the operation changes something");
    content.apply(&operation);
    step
}

fn reverted<C: Undo>(content: &mut C, step: &C::Step) {
    for operation in content.revert(step) {
        content.apply(&operation);
    }
}

fn logic(content: &LogicGridContent, operations: &[LogicGridOperation]) -> Edit {
    content.root().edit_for_all(operations)
}

fn logic_run(content: &LogicGridContent, operations: &[LogicGridOperation]) -> LogicGridContent {
    edited(content, [logic(content, operations)])
}

fn logic_wire(start: (i64, i64), end: (i64, i64)) -> Wire {
    Wire::new(
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        Scale::ONE,
    )
    .expect("a straight wire at least one cell long")
}

fn not_gate(id: ComponentId, x: i64, y: i64) -> Component {
    Component {
        id,
        position: Point::new(x, y),
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::Not { scale: Scale::ONE },
    }
}
