use std::cell::Cell;
use std::rc::Rc;

use be_block::{BlockContent, Checklist, ChecklistContent, Edit, LiveEdit, ObjectId, Touched};
use beui::reactive::Scope;

use super::ContentProjection;
use crate::EditorHost;

mod a_foreign_edit_during_an_edit_in_flight_keeps_both;
mod an_edit_coming_back_as_mine_runs_nothing_again;
mod an_operation_runs_only_the_watchers_of_what_it_touched;

struct Fixture {
    host: EditorHost,
    projection: ContentProjection<ChecklistContent>,
    first: ObjectId,
    second: ObjectId,
    runs: [Rc<Cell<u32>>; 3],
    _scope: Scope,
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
        host.set_block_content(ChecklistContent::CONTENT_TYPE, content.encode(), 0);
        let projection = ContentProjection::<ChecklistContent>::new(host.clone());
        let runs = [
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        ];
        let scope = Scope::new();
        scope.run(|| {
            for (key, count) in [
                Touched::Subtree(first),
                Touched::Subtree(second),
                Touched::Field(ObjectId::ROOT, Checklist::ITEMS.index()),
            ]
            .into_iter()
            .zip(runs.iter().cloned())
            {
                let _ = projection.project_on(key, move |_| count.set(count.get() + 1));
            }
        });
        projection.pump();
        let fixture = Self {
            host,
            projection,
            first,
            second,
            runs,
            _scope: scope,
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
                .map(|(edit, mine)| (ChecklistContent::encode_operation(edit), *mine))
                .collect(),
        );
        self.projection.pump();
    }

    fn text(&self, id: ObjectId) -> String {
        self.projection
            .read(|content| content.root().items.get(id).map(|item| item.text.clone()))
            .flatten()
            .unwrap_or_default()
    }
}
