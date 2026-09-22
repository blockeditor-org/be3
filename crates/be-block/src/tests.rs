use super::*;
use crate::{
    counter::{CounterContent, CounterOp},
    image::{ImageContent, ImageHeader},
    text::{TextContent, TextLanguage, TextOp},
};

mod a_counter_reset_wins_over_the_adds_before_it;
mod a_counter_round_trips_through_its_bytes;
mod an_image_merges_only_when_one_side_changed_it;
mod streamed_content_separates_its_header_from_its_payload;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_operations_rebase_onto_concurrent_edits;
mod two_counters_merge_by_keeping_both_sides_of_the_count;

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
