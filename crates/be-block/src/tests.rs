use super::*;
use crate::{
    browser_tab::{BrowserTabContent, BrowserTabOp, HistoryItem},
    calendar::{CalendarContent, CalendarEvent, CalendarOp, CalendarStep},
    checklist::{ChecklistContent, ChecklistOp},
    counter::{CounterContent, CounterOp},
    image::{ImageContent, ImageHeader},
    text::{TextContent, TextLanguage, TextOp},
    ui_settings::{UiSettingsContent, UiSettingsOp},
};

mod a_browser_tab_is_named_after_its_page_and_refuses_a_bad_index;
mod a_browser_tab_push_discards_forward_history;
mod a_burst_of_edits_to_one_event_undoes_as_one_step;
mod a_calendar_undo_keeps_what_someone_else_changed_since;
mod a_checklist_ignores_a_second_add_of_one_item;
mod a_checklist_round_trips_through_its_bytes;
mod a_counter_reset_wins_over_the_adds_before_it;
mod a_counter_round_trips_through_its_bytes;
mod an_image_merges_only_when_one_side_changed_it;
mod browser_tabs_navigated_on_both_sides_conflict;
mod calendars_merge_each_event_field_by_field;
mod streamed_content_separates_its_header_from_its_payload;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_operations_rebase_onto_concurrent_edits;
mod two_checklists_conflict_only_where_both_changed_one_field;
mod two_checklists_merge_every_item_either_side_added;
mod two_counters_merge_by_keeping_both_sides_of_the_count;
mod ui_settings_keep_the_zoom_in_bounds;

fn header(name: &str) -> ImageHeader {
    ImageHeader {
        source_name: name.into(),
        media_type: "image/png".into(),
        width: 640,
        height: 480,
    }
}

fn image(name: &str, payload: &[u8]) -> ImageContent {
    ImageContent::new(header(name), payload.to_vec())
}

fn applied(start: &str, operations: &[TextOp]) -> String {
    let mut content = TextContent::from(start);
    for operation in operations {
        content.apply(operation);
    }
    content.text()
}

fn counted(start: CounterContent, operations: &[CounterOp]) -> CounterContent {
    let mut content = start;
    for operation in operations {
        content.apply(operation);
    }
    content
}

fn listed(start: &ChecklistContent, operations: &[ChecklistOp]) -> ChecklistContent {
    let mut content = start.clone();
    for operation in operations {
        content.apply(operation);
    }
    content
}

fn texts(checklist: &ChecklistContent) -> Vec<(&str, bool)> {
    checklist
        .items()
        .iter()
        .map(|item| (item.text.as_str(), item.done))
        .collect()
}

fn meeting() -> CalendarEvent {
    CalendarEvent::new("Standup".to_owned(), 540, 555)
}

fn scheduled(event: &CalendarEvent) -> CalendarContent {
    let mut calendar = CalendarContent::default();
    calendar.apply(&CalendarOp::AddEvent {
        event: event.clone(),
    });
    calendar
}
