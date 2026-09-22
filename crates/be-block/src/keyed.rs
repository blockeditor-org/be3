use std::collections::{BTreeMap, BTreeSet};

use be_commit::{MergeResult, merge_slices};
use uuid::Uuid;

pub(crate) fn merge_keyed<T: Clone + PartialEq>(
    base: &[T],
    ours: &[T],
    theirs: &[T],
    id: impl Fn(&T) -> Uuid,
    fields: impl Fn(&T, &T, &T, &mut usize) -> T,
) -> MergeResult<Vec<T>> {
    let by_id = |items: &[T]| -> BTreeMap<Uuid, T> {
        items.iter().map(|item| (id(item), item.clone())).collect()
    };
    let ids = |items: &[T]| -> Vec<Uuid> { items.iter().map(&id).collect() };
    let (base_items, ours_items, theirs_items) = (by_id(base), by_id(ours), by_id(theirs));
    let mut conflicts = 0;
    let mut kept = BTreeMap::new();
    let every: BTreeSet<Uuid> = base_items
        .keys()
        .chain(ours_items.keys())
        .chain(theirs_items.keys())
        .copied()
        .collect();
    for key in every {
        let resolved = merge_item(
            base_items.get(&key),
            ours_items.get(&key),
            theirs_items.get(&key),
            &fields,
            &mut conflicts,
        );
        if let Some(item) = resolved {
            kept.insert(key, item);
        }
    }

    let outcome = merge_slices(&ids(base), &ids(ours), &ids(theirs));
    let mut order = outcome.merged;
    for conflict in outcome.conflicts.iter().rev() {
        let after_ours = conflict.at + conflict.ours.len();
        let only_theirs = conflict
            .theirs
            .iter()
            .filter(|key| !conflict.ours.contains(key))
            .copied();
        order.splice(after_ours..after_ours, only_theirs);
    }
    let mut items = Vec::with_capacity(kept.len());
    for key in order {
        if let Some(item) = kept.remove(&key) {
            items.push(item);
        }
    }
    items.extend(kept.into_values());

    match conflicts {
        0 => MergeResult::Clean(items),
        conflicts => MergeResult::Conflicted {
            value: items,
            conflicts,
        },
    }
}

fn merge_item<T: Clone + PartialEq>(
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
    fields: impl Fn(&T, &T, &T, &mut usize) -> T,
    conflicts: &mut usize,
) -> Option<T> {
    if ours == base {
        return theirs.cloned();
    }
    if theirs == base || theirs == ours {
        return ours.cloned();
    }
    match (base, ours, theirs) {
        (Some(base), Some(ours), Some(theirs)) => Some(fields(base, ours, theirs, conflicts)),
        (_, Some(ours), _) => {
            *conflicts += 1;
            Some(ours.clone())
        }
        (_, None, theirs) => {
            *conflicts += 1;
            theirs.cloned()
        }
    }
}

pub(crate) fn pick<T: Clone + PartialEq>(
    base: &T,
    ours: &T,
    theirs: &T,
    conflicts: &mut usize,
) -> T {
    if ours == base {
        theirs.clone()
    } else if theirs == base || theirs == ours {
        ours.clone()
    } else {
        *conflicts += 1;
        ours.clone()
    }
}
