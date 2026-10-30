use std::cell::Cell;
use std::rc::Rc;

use be_block::{Edit, LiveEdit, ObjectId, Touched};
use checklist_types::{Checklist, ChecklistContent};

use super::LiveContent;
use crate::Subscription;
use crate::{EditorHost, HostContent};

fn arrived(operation: Vec<u8>, mine: bool) -> crate::EditorOperation {
    crate::EditorOperation {
        operation,
        mine,
        unsettled: 0,
        beneath: None,
    }
}

mod a_foreign_edit_during_an_edit_in_flight_keeps_both;
mod a_foreign_edit_during_an_edit_in_flight_runs_only_what_either_touched;
mod a_peer_edit_beneath_the_hosts_unsettled_edits_runs_only_what_either_touched;
mod a_snapshot_carries_where_removed_items_were;
mod an_edit_before_the_content_arrives_is_dropped;
mod an_edit_coming_back_as_mine_runs_nothing_again;
mod an_edit_rereads_only_the_objects_it_touched;
mod an_operation_runs_only_the_watchers_of_what_it_touched;
mod an_undecodable_operation_asks_for_a_fresh_snapshot;
mod updates_for_another_block_reach_only_its_projection;

struct Fixture {
    host: EditorHost,
    projection: LiveContent<ChecklistContent>,
    first: ObjectId,
    second: ObjectId,
    runs: [Rc<Cell<u32>>; 3],
    _watching: Vec<Subscription>,
}

impl Fixture {
    fn new() -> Self {
        let host = EditorHost::default();
        host.set_editable(true);
        let (first, add_first) = Checklist::add("milk");
        let (second, add_second) = Checklist::add("eggs");
        let mut content = ChecklistContent::default();
        content.apply(&add_first);
        content.apply(&add_second);
        host.set_block_content(HostContent::of(&content));
        let projection = LiveContent::<ChecklistContent>::new(host.clone(), None);
        let runs = [
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        ];
        let watching = [
            Touched::Subtree(first),
            Touched::Subtree(second),
            Touched::Field(ObjectId::ROOT, Checklist::ITEMS.index()),
        ]
        .into_iter()
        .zip(runs.iter().cloned())
        .map(|(key, count)| projection.on_touched(key, move |_, _| count.set(count.get() + 1)))
        .collect();
        projection.pump();
        assert_eq!(host.take_content_resend_requests(), [None]);
        let fixture = Self {
            host,
            projection,
            first,
            second,
            runs,
            _watching: watching,
        };
        fixture.reset();
        fixture
    }

    fn reset(&self) {
        for count in &self.runs {
            count.set(0);
        }
    }

    fn runs(&self) -> [u32; 3] {
        self.runs.clone().map(|count| count.get())
    }

    fn arrive(&self, edits: &[(Edit, bool)]) {
        self.host.push_content_operations(
            edits
                .iter()
                .map(|(edit, mine)| arrived(ChecklistContent::encode_operation(edit), *mine))
                .collect(),
        );
        self.projection.pump();
    }

    fn text(&self, id: ObjectId) -> String {
        self.projection
            .read(|content| content.root().items.get(id).map(|item| item.text.clone()))
            .ok()
            .flatten()
            .unwrap_or_default()
    }
}
