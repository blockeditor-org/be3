use super::*;
use crate::{
    browser_tab::{BrowserTabContent, HistoryItem},
    calendar::{Calendar, CalendarContent, CalendarEvent},
    checklist::{Checklist, ChecklistContent},
    counter::{Counter, CounterContent},
    image::{ImageContent, ImageHeader},
    text::{TextContent, TextLanguage, TextOp},
    ui_settings::{UiSettings, UiSettingsContent},
};

mod a_browser_tab_is_named_after_its_page;
mod a_browser_tab_push_discards_forward_history;
mod a_calendar_undo_keeps_what_someone_else_changed_since;
mod a_calendar_update_writes_only_the_fields_that_changed;
mod a_counter_reset_undoes_back_to_its_count;
mod an_image_merges_only_when_one_side_changed_it;
mod calendars_merge_each_event_field_by_field;
mod clearing_a_checklist_keeps_the_open_items;
mod streamed_content_separates_its_header_from_its_payload;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_operations_rebase_onto_concurrent_edits;
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

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn scheduled() -> (CalendarContent, crate::ObjectId) {
    let (id, add) = Calendar::add(&CalendarEvent::new("Standup", 540, 555));
    (edited(&CalendarContent::default(), [add]), id)
}
