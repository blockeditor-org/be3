//! Moving a pending edit across a session restart
//! (decisions/sessions_restart_when_their_state_outgrows_the_content.md): the same visible
//! content loaded afresh has new positions, so whatever names positions travels as what it
//! visibly did.

use be_lib_sequence::marks::{Mark, Marks, Rich, Seed};
use be_lib_sequence::{SeqOp, Sequence, Splice};

use crate::{Change, Edit, Model, Value};

impl<R: Model> crate::Document<R> {
    /// `edit`, made on `before`, as `client` applies it to `after`: the same visible content
    /// loaded afresh. A text or series operation is rebuilt from the visible splices it makes on
    /// `before` (what it shows inserted by `client`), a mark from the formats it leaves, and
    /// every other change names objects, which keep their ids, and travels unchanged. None when
    /// nothing of it is left.
    pub fn carry_over(edit: &Edit, before: &Self, after: &Self, client: u64) -> Option<Edit> {
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
                    rebuilt(old, new, op, client)
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
                    rebuilt(old, new, op, client)
                        .into_iter()
                        .map(|op| Change::Series {
                            object: *object,
                            field: *field,
                            op,
                        })
                        .collect()
                }
                Change::Mark { object, field, .. } | Change::Unmark { object, field, .. } => {
                    let (Some(Value::Rich(old)), Some(Value::Rich(new))) = (
                        before.tree.value(*object, *field),
                        after.tree.value(*object, *field),
                    ) else {
                        continue;
                    };
                    let mut changed = old.clone();
                    let key = match change {
                        Change::Mark { mark, .. } => {
                            changed.mark(mark.clone());
                            mark.op.key.clone()
                        }
                        Change::Unmark { id, .. } => {
                            let Some(key) = old.marks.get(*id).map(|mark| mark.op.key.clone())
                            else {
                                continue;
                            };
                            changed.unmark(*id);
                            key
                        }
                        _ => continue,
                    };
                    repainted(old, &changed, new, key.as_str(), client)
                        .into_iter()
                        .map(|mark| Change::Mark {
                            object: *object,
                            field: *field,
                            mark,
                        })
                        .collect()
                }
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

/// Where an item of the content after an operation came from: the visible content before it,
/// or what one of its splices showed.
#[derive(Clone, Copy)]
enum Came {
    Kept,
    Shown(usize),
}

/// `op` on `old` as operations on `new`, which shows the same items: each visible splice it
/// made, replayed as a delete and an insert by `client` of what it showed.
fn rebuilt<T: Clone>(
    old: &Sequence<T>,
    new: &Sequence<T>,
    op: &SeqOp<T>,
    client: u64,
) -> Vec<SeqOp<T>> {
    let mut old = old.clone();
    let mut new = new.clone();
    let mut mirror = vec![Came::Kept; old.len()];
    let Some(splices) = old.apply(op) else {
        return Vec::new();
    };
    for (index, splice) in splices.iter().enumerate() {
        mirror.splice(
            splice.at..splice.at + splice.removed,
            std::iter::repeat_n(Came::Shown(index), splice.inserted),
        );
    }
    let shown = old.items();
    let mut ops = Vec::new();
    for (index, Splice { at, removed, .. }) in splices.into_iter().enumerate() {
        if removed > 0
            && let Some(delete) = new.delete(at..at + removed)
        {
            new.apply(&delete);
            ops.push(delete);
        }
        let items: Vec<T> = mirror
            .iter()
            .zip(&shown)
            .filter(|(came, _)| matches!(came, Came::Shown(from) if *from == index))
            .map(|(_, item)| item.clone())
            .collect();
        if let Some(insert) = new.insert(client, at, items) {
            new.apply(&insert);
            ops.push(insert);
        }
    }
    ops
}

/// Marks that give `new` the formats of `key` that `changed` has and `old` lacked: each run the
/// change painted, and each it cleared.
fn repainted(old: &Rich, changed: &Rich, new: &Rich, key: &str, client: u64) -> Vec<Mark> {
    let of_key = |rich: &Rich| -> Vec<Seed> {
        rich.seeds()
            .into_iter()
            .filter(|seed| seed.key.as_str() == key)
            .collect()
    };
    let before = of_key(old);
    let after = of_key(changed);
    let mut new = new.clone();
    let bytes = new.text.items();
    let cleared = before
        .iter()
        .filter(|seed| !after.contains(seed))
        .map(|seed| (seed, None));
    let painted = after
        .iter()
        .filter(|seed| !before.contains(seed))
        .map(|seed| (seed, Some(seed.value.clone())));
    let mut marks = Vec::new();
    for (seed, value) in cleared.chain(painted) {
        let Some(op) = Marks::op(
            &new.text,
            &bytes,
            seed.range.clone(),
            seed.scope,
            seed.expand,
            &seed.key,
            value,
        ) else {
            continue;
        };
        marks.push(new.marks.mint(&new.text, client, op));
    }
    marks
}
