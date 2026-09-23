use std::collections::{BTreeMap, BTreeSet, HashSet};

use be_commit::merge_slices;

use crate::{Object, ObjectId, Objects, Place, Tree, Value};

pub(crate) fn merge3(base: &Tree, ours: &Tree, theirs: &Tree) -> (Tree, usize) {
    let mut conflicts = 0;
    let ids: BTreeSet<ObjectId> = base
        .objects()
        .keys()
        .chain(ours.objects().keys())
        .chain(theirs.objects().keys())
        .copied()
        .collect();
    let mut kept = Objects::new();
    for id in ids {
        let resolved = merge_object(
            base.object(id),
            ours.object(id),
            theirs.object(id),
            &mut conflicts,
        );
        if let Some(object) = resolved {
            kept.insert(id, object);
        }
    }
    if !kept.contains_key(&ObjectId::ROOT)
        && let Some(root) = ours.object(ObjectId::ROOT)
    {
        kept.insert(ObjectId::ROOT, root.clone());
    }
    restore_ancestors(&mut kept, [ours, theirs, base], &mut conflicts);
    let mut merged = Tree::from_map(kept);
    rebuild_lists(&mut merged, base, ours, theirs);
    (merged, conflicts)
}

fn merge_object(
    base: Option<&Object>,
    ours: Option<&Object>,
    theirs: Option<&Object>,
    conflicts: &mut usize,
) -> Option<Object> {
    if ours == base {
        return theirs.cloned();
    }
    if theirs == base || theirs == ours {
        return ours.cloned();
    }
    match (base, ours, theirs) {
        (Some(base), Some(ours), Some(theirs)) => Some(merge_fields(base, ours, theirs, conflicts)),
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

fn merge_fields(base: &Object, ours: &Object, theirs: &Object, conflicts: &mut usize) -> Object {
    let parent = pick(&base.parent, &ours.parent, &theirs.parent, conflicts);
    let fields = ours
        .fields
        .iter()
        .enumerate()
        .map(|(index, mine)| {
            let (Some(before), Some(other)) = (base.fields.get(index), theirs.fields.get(index))
            else {
                return mine.clone();
            };
            match (before, mine, other) {
                (Value::Count(before), Value::Count(mine), Value::Count(other)) => Value::Count(
                    before
                        .saturating_add(mine.saturating_sub(*before))
                        .saturating_add(other.saturating_sub(*before)),
                ),
                (Value::Register(_), Value::Register(_), Value::Register(_)) => {
                    pick(before, mine, other, conflicts)
                }
                (Value::Map(before), Value::Map(mine), Value::Map(other)) => {
                    Value::Map(merge_entries(before, mine, other, conflicts))
                }
                _ => mine.clone(),
            }
        })
        .collect();
    Object { parent, fields }
}

fn merge_entries(
    base: &BTreeMap<Vec<u8>, Vec<u8>>,
    ours: &BTreeMap<Vec<u8>, Vec<u8>>,
    theirs: &BTreeMap<Vec<u8>, Vec<u8>>,
    conflicts: &mut usize,
) -> BTreeMap<Vec<u8>, Vec<u8>> {
    let keys: BTreeSet<&Vec<u8>> = base.keys().chain(ours.keys()).chain(theirs.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            pick(&base.get(key), &ours.get(key), &theirs.get(key), conflicts)
                .map(|value| (key.clone(), value.clone()))
        })
        .collect()
}

fn pick<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T, conflicts: &mut usize) -> T {
    if ours == base {
        theirs.clone()
    } else if theirs == base || theirs == ours {
        ours.clone()
    } else {
        *conflicts += 1;
        ours.clone()
    }
}

fn restore_ancestors(kept: &mut Objects, versions: [&Tree; 3], conflicts: &mut usize) {
    let ids: Vec<ObjectId> = kept.keys().copied().collect();
    for id in ids {
        let mut seen = HashSet::new();
        let mut cursor = id;
        loop {
            if !seen.insert(cursor) {
                let fallback = versions
                    .iter()
                    .rev()
                    .find_map(|version| version.object(id))
                    .and_then(|object| object.parent);
                if let Some(object) = kept.get_mut(&id) {
                    object.parent = fallback;
                }
                *conflicts += 1;
                break;
            }
            let Some(parent) = kept.get(&cursor).and_then(|object| object.parent) else {
                break;
            };
            if !kept.contains_key(&parent.object) {
                let Some(restored) = versions
                    .iter()
                    .find_map(|version| version.object(parent.object))
                else {
                    kept.remove(&id);
                    break;
                };
                kept.insert(parent.object, restored.clone());
                *conflicts += 1;
            }
            cursor = parent.object;
        }
    }
    let orphans: Vec<ObjectId> = kept
        .iter()
        .filter(|(id, object)| **id != ObjectId::ROOT && object.parent.is_none())
        .map(|(id, _)| *id)
        .collect();
    for id in orphans {
        kept.remove(&id);
    }
}

fn rebuild_lists(merged: &mut Tree, base: &Tree, ours: &Tree, theirs: &Tree) {
    let children = merged.children_by_place();
    let mut places: Vec<Place> = Vec::new();
    for (id, object) in merged.objects() {
        for (index, value) in object.fields.iter().enumerate() {
            if let (Value::List(_), Ok(field)) = (value, u16::try_from(index)) {
                places.push(Place { object: *id, field });
            }
        }
    }
    for place in places {
        let sequence = |tree: &Tree| -> Vec<ObjectId> {
            match tree
                .object(place.object)
                .and_then(|object| object.fields.get(usize::from(place.field)))
            {
                Some(Value::List(items)) => items.clone(),
                _ => Vec::new(),
            }
        };
        let (before, mine, other) = (sequence(base), sequence(ours), sequence(theirs));
        let outcome = merge_slices(&before, &mine, &other);
        let mut order = outcome.merged;
        for conflict in outcome.conflicts.iter().rev() {
            let after_ours = conflict.at + conflict.ours.len();
            let only_theirs = conflict
                .theirs
                .iter()
                .filter(|id| !conflict.ours.contains(id))
                .copied();
            order.splice(after_ours..after_ours, only_theirs);
        }
        let belongs: BTreeSet<ObjectId> = children
            .get(&place)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default();
        let mut placed = BTreeSet::new();
        let mut items: Vec<ObjectId> = order
            .into_iter()
            .chain(mine)
            .chain(other)
            .filter(|id| belongs.contains(id) && placed.insert(*id))
            .collect();
        items.extend(belongs.iter().filter(|id| placed.insert(**id)));
        merged.set_list(place, items);
    }
}
