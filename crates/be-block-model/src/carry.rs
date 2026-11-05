//! Moving a pending edit across a session restart (plans: guides/blocks/client.md, "Session
//! restarts"): the same visible content loaded afresh has new positions, so a sequence
//! operation travels as the visible splices it made.

use be_lib_sequence::{SeqOp, Sequence, Splice};

use crate::{Change, Edit, Model, Value};

impl<R: Model> crate::Document<R> {
    /// `edit`, made on `before`, as it applies to `after`: the same visible content loaded
    /// afresh. Text and series operations are rebuilt from the visible splices they make on
    /// `before`; marks, which name positions, are dropped; every other change names objects,
    /// which keep their ids, and travels unchanged. None when nothing of it is left.
    pub fn carry_over(edit: &Edit, before: &Self, after: &Self) -> Option<Edit> {
        let mut before = before.clone();
        let mut after = after.clone();
        let mut carried = Vec::new();
        for change in &edit.0 {
            let moved = match change {
                Change::Text { object, field, op } => {
                    let (Some(old), Some(new)) = (
                        text_of(before.tree.value(*object, *field)),
                        text_of(after.tree.value(*object, *field)),
                    ) else {
                        continue;
                    };
                    rebuilt(old, new, op)
                        .into_iter()
                        .map(|op| Change::Text {
                            object: *object,
                            field: *field,
                            op,
                        })
                        .collect()
                }
                Change::Series { object, field, op } => {
                    let (Some(Value::Series(old)), Some(Value::Series(new))) = (
                        before.tree.value(*object, *field),
                        after.tree.value(*object, *field),
                    ) else {
                        continue;
                    };
                    rebuilt(old, new, op)
                        .into_iter()
                        .map(|op| Change::Series {
                            object: *object,
                            field: *field,
                            op,
                        })
                        .collect()
                }
                Change::Mark { .. } | Change::Unmark { .. } => continue,
                change => vec![change.clone()],
            };
            before.change(change);
            for change in &moved {
                after.change(change);
            }
            carried.extend(moved);
        }
        (!carried.is_empty()).then_some(Edit(carried))
    }
}

fn text_of(value: Option<&Value>) -> Option<&Sequence<u8>> {
    match value? {
        Value::Text(sequence) => Some(sequence),
        Value::Rich(rich) => Some(&rich.text),
        _ => None,
    }
}

/// `op` on `old` as operations on `new`, which shows the same items: its visible splices
/// replayed. Inserts, deletes and replaces travel; an undelete, swap or move, which brings back
/// or moves items by position, does not.
fn rebuilt<T: Clone>(old: &Sequence<T>, new: &Sequence<T>, op: &SeqOp<T>) -> Vec<SeqOp<T>> {
    let (client, mut items) = match op {
        SeqOp::Insert { client, items, .. } | SeqOp::Replace { client, items, .. } => {
            (Some(*client), items.clone())
        }
        SeqOp::Delete { .. } => (None, Vec::new()),
        SeqOp::Undelete { .. } | SeqOp::Swap { .. } | SeqOp::Move { .. } => return Vec::new(),
    };
    let mut old = old.clone();
    let mut new = new.clone();
    let Some(splices) = old.apply(op) else {
        return Vec::new();
    };
    let mut ops = Vec::new();
    for Splice {
        at,
        removed,
        inserted,
    } in splices
    {
        if removed > 0
            && let Some(delete) = new.delete(at..at + removed)
        {
            new.apply(&delete);
            ops.push(delete);
        }
        if inserted > 0
            && let Some(client) = client
        {
            let shown: Vec<T> = items.drain(..inserted.min(items.len())).collect();
            if let Some(insert) = new.insert(client, at, shown) {
                new.apply(&insert);
                ops.push(insert);
            }
        }
    }
    ops
}
