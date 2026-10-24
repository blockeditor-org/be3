use be_block::{Edit, LiveEdit, Merge, Undo};
use be_commit::MergeResult;
use be_model::ObjectId;

use crate::{Calendar, CalendarContent, CalendarEvent};

mod a_calendar_undo_keeps_what_someone_else_changed_since;
mod a_calendar_update_writes_only_the_fields_that_changed;
mod a_cancelled_gesture_is_reverted_and_leaves_no_step;
mod a_gesture_is_one_undo_step_however_long_it_takes;
mod an_event_rescheduled_on_both_sides_conflicts_and_keeps_ours;
mod an_update_for_an_event_someone_removed_does_not_bring_it_back;
mod calendars_merge_each_event_field_by_field;
mod undoing_an_event_removal_restores_it_with_its_id;

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

fn scheduled() -> (CalendarContent, ObjectId) {
    let (id, add) = Calendar::add(&CalendarEvent::new("Standup", 540, 555));
    (edited(&CalendarContent::default(), [add]), id)
}
