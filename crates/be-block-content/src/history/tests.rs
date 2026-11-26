use std::collections::BTreeMap;

use uuid::Uuid;

use super::*;
use crate::{BlockContent, ContentError, LiveEdit};

mod a_cancel_ends_only_the_gesture_it_names;
mod a_gesture_named_again_commits_the_open_one_first;
mod a_restart_keeps_open_gestures_whole;
mod a_split_leaves_an_open_group_whole;
mod an_edit_outside_any_open_gesture_is_its_own_step;
mod another_editors_gesture_does_not_commit_an_open_one;
mod cancelling_one_tentative_gesture_keeps_the_rest_held;
mod clearing_ends_every_gesture;
mod ending_a_gesture_that_is_not_open_changes_nothing;
mod held_edits_wait_while_ordinary_edits_step_beneath_them;
mod tentative_gestures_commit_as_one_step_when_the_last_ends;
mod undo_commits_open_undo_groups_first;
mod undo_while_held_cancels_the_layer_and_redo_is_refused;

/// Numbered cells, each holding a number or nothing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Cells(BTreeMap<u32, i64>);

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
struct Set(u32, Option<i64>);

impl BlockContent for Cells {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6365_6c6c_732d_6869_7374_6f72_792d_7465);

    fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(&self.0.iter().collect::<Vec<_>>()).unwrap()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        postcard::from_bytes::<Vec<(u32, i64)>>(bytes)
            .map(|cells| Self(cells.into_iter().collect()))
            .map_err(|_| ContentError::Malformed("cells"))
    }
}

impl LiveEdit for Cells {
    type Op = Set;

    fn apply(&mut self, Set(cell, value): &Set) {
        match value {
            Some(value) => self.0.insert(*cell, *value),
            None => self.0.remove(cell),
        };
    }
}

/// Each change as the cell, what it held before and what after.
type Changes = Vec<(u32, Option<i64>, Option<i64>)>;

impl Undo for Cells {
    type Step = Changes;

    fn step(&self, Set(cell, value): &Set) -> Option<Changes> {
        Some(vec![(*cell, self.0.get(cell).copied(), *value)])
    }

    fn then(previous: &mut Changes, next: Changes) {
        previous.extend(next);
    }

    fn revert(&self, step: &Changes) -> Vec<Set> {
        step.iter()
            .rev()
            .map(|(cell, before, _)| Set(*cell, *before))
            .collect()
    }

    fn reapply(&self, step: &Changes) -> Vec<Set> {
        step.iter()
            .map(|(cell, _, after)| Set(*cell, *after))
            .collect()
    }
}

fn gesture(editor: u64, id: u64) -> GestureId {
    GestureId {
        editor: Some(editor),
        id,
    }
}

/// A replica in miniature, as guides/type_modules.md (Tentative gestures) shapes it: `written`
/// is what was written, and the tentative layer's edits sit on it, held, until the layer commits.
#[derive(Default)]
struct Desk {
    written: Cells,
    layer: Vec<(GestureId, Set)>,
    history: History<Cells, u64>,
    now: u64,
}

impl Desk {
    /// What every view shows: what was written, then the layer.
    fn visible(&self) -> Vec<(u32, i64)> {
        let mut visible = self.written.clone();
        for (_, operation) in &self.layer {
            visible.apply(operation);
        }
        visible.0.into_iter().collect()
    }

    fn written(&self) -> Vec<(u32, i64)> {
        self.written.0.clone().into_iter().collect()
    }

    fn set(&mut self, editor: u64, id: Option<u64>, cell: u32, value: i64) {
        let operation = Set(cell, Some(value));
        let held = id.map(|id| gesture(editor, id));
        match held.filter(|held| self.history.tentative().contains(held)) {
            Some(held) => {
                let mut visible = self.written.clone();
                for (_, operation) in &self.layer {
                    visible.apply(operation);
                }
                self.history
                    .record(&visible, &operation, Some(editor), id, Some(self.now));
                self.layer.push((held, operation));
            }
            None => {
                self.history
                    .record(&self.written, &operation, Some(editor), id, Some(self.now));
                self.written.apply(&operation);
            }
        }
    }

    /// A begin under a name already open commits that gesture first, as the replica does.
    fn begin(&mut self, editor: u64, id: u64, kind: GestureKind) {
        self.commit(editor, id);
        self.history.begin(gesture(editor, id), kind);
    }

    fn commit(&mut self, editor: u64, id: u64) {
        self.history.commit(gesture(editor, id));
        self.settle();
    }

    fn cancel(&mut self, editor: u64, id: u64) {
        let ended = gesture(editor, id);
        for operation in self.history.cancel(&self.written, ended) {
            self.written.apply(&operation);
        }
        self.layer.retain(|(of, _)| *of != ended);
        self.settle();
    }

    fn undo(&mut self) -> bool {
        let stepped = self.history.undo(&self.written);
        self.step(stepped)
    }

    fn redo(&mut self) -> bool {
        let stepped = self.history.redo(&self.written);
        self.step(stepped)
    }

    fn step(&mut self, stepped: Stepped<Set>) -> bool {
        let changed = stepped.changed();
        if !stepped.cancelled.is_empty() {
            self.layer.clear();
        }
        for operation in stepped.operations {
            self.written.apply(&operation);
        }
        changed
    }

    /// Undoes until nothing is left, collecting what each undo leaves.
    fn undo_all(&mut self) -> Vec<Vec<(u32, i64)>> {
        let mut seen = Vec::new();
        while self.undo() {
            seen.push(self.visible());
        }
        seen
    }

    /// Once no tentative gesture is open, the layer is written, in the order it was made.
    fn settle(&mut self) {
        if !self.history.holding() {
            for (_, operation) in std::mem::take(&mut self.layer) {
                self.written.apply(&operation);
            }
        }
    }
}
