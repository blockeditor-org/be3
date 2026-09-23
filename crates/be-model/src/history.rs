use crate::{Change, Edit, Tree};

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
        let chained = self.redo.len() == next.redo.len()
            && self.redo.iter().zip(&next.redo).all(|(done, following)| {
                matches!(
                    (done, following),
                    (
                        Change::SetIf { object, field, value, .. },
                        Change::SetIf { object: next_object, field: next_field, expected, .. },
                    ) if object == next_object && field == next_field && value == expected
                )
            });
        if !chained {
            return Err(next);
        }
        for (undo, latest) in self.undo.iter_mut().zip(next.undo) {
            if let (
                Change::SetIf { expected, .. },
                Change::SetIf {
                    expected: after, ..
                },
            ) = (undo, latest)
            {
                *expected = after;
            }
        }
        for (redo, latest) in self.redo.iter_mut().zip(next.redo) {
            if let (Change::SetIf { value, .. }, Change::SetIf { value: after, .. }) =
                (redo, latest)
            {
                *value = after;
            }
        }
        Ok(())
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
