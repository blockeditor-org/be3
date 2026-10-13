use crate::{Change, Edit, SeqOp, Tree};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    undo: Vec<Change>,
    redo: Vec<Change>,
}

impl Step {
    pub fn undo(&self) -> Edit {
        self.undo.iter().rev().cloned().collect()
    }

    pub fn redo(&self) -> Edit {
        Edit(self.redo.clone())
    }

    pub fn absorb(&mut self, next: Self) -> Result<(), Self> {
        if strokes(&self.redo, &next.redo) || typing(&self.redo, &next.redo) {
            self.undo.extend(next.undo);
            self.redo.extend(next.redo);
            return Ok(());
        }
        let chained = self.redo.len() == next.redo.len()
            && self
                .redo
                .iter()
                .zip(&next.redo)
                .all(|(done, following)| follows(done, following));
        if !chained {
            return Err(next);
        }
        for (undo, latest) in self.undo.iter_mut().zip(next.undo) {
            match (undo, latest) {
                (
                    Change::SetIf { expected, .. },
                    Change::SetIf {
                        expected: after, ..
                    },
                ) => {
                    *expected = after;
                }
                (
                    Change::PutIf { expected, .. },
                    Change::PutIf {
                        expected: after, ..
                    },
                ) => {
                    *expected = after;
                }
                _ => {}
            }
        }
        for (redo, latest) in self.redo.iter_mut().zip(next.redo) {
            match (redo, latest) {
                (Change::SetIf { value, .. }, Change::SetIf { value: after, .. }) => {
                    *value = after;
                }
                (Change::PutIf { value, .. }, Change::PutIf { value: after, .. }) => {
                    *value = after;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn strokes(done: &[Change], following: &[Change]) -> bool {
    let Some(Change::Paint { object, field, .. }) = done.first() else {
        return false;
    };
    done.iter().chain(following).all(|change| {
        matches!(
            change,
            Change::Paint {
                object: painted,
                field: into,
                ..
            } if painted == object && into == field
        )
    })
}

fn typing(done: &[Change], following: &[Change]) -> bool {
    let Some(Change::Text { object, field, op }) = done.first() else {
        return false;
    };
    let deleting = matches!(op, SeqOp::Delete { .. });
    let burst = |op: &SeqOp<u8>| match deleting {
        true => matches!(op, SeqOp::Delete { .. }),
        false => matches!(op, SeqOp::Undelete { .. }),
    };
    done.iter().chain(following).all(|change| {
        matches!(
            change,
            Change::Text {
                object: edited,
                field: into,
                op,
            } if edited == object && into == field && burst(op)
        )
    })
}

fn follows(done: &Change, following: &Change) -> bool {
    match (done, following) {
        (
            Change::SetIf {
                object,
                field,
                value,
                ..
            },
            Change::SetIf {
                object: next_object,
                field: next_field,
                expected,
                ..
            },
        ) => object == next_object && field == next_field && value == expected,
        (
            Change::PutIf {
                object,
                field,
                key,
                value,
                ..
            },
            Change::PutIf {
                object: next_object,
                field: next_field,
                key: next_key,
                expected,
                ..
            },
        ) => object == next_object && field == next_field && key == next_key && value == expected,
        _ => false,
    }
}

pub(crate) fn step(tree: &Tree, edit: &Edit) -> Option<Step> {
    let mut undo = Vec::new();
    let mut redo = Vec::new();
    match edit.0.as_slice() {
        [] => return None,
        [change] => {
            let (back, forward) = tree.inverse(change)?;
            undo.push(back);
            redo.push(forward);
        }
        changes => {
            let mut scratch = tree.clone();
            for change in changes {
                if let Some((back, forward)) = scratch.inverse(change) {
                    undo.push(back);
                    redo.push(forward);
                }
                scratch.apply(change);
            }
        }
    }
    (!redo.is_empty()).then_some(Step { undo, redo })
}
