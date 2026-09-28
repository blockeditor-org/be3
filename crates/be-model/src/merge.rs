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
    let mut rescued = BTreeSet::new();
    let mut dropped = Vec::new();
    for id in ids {
        let resolved = match (base.object(id), ours.object(id), theirs.object(id)) {
            (Some(before), None, Some(other)) => {
                removed_or_moved(id, before, other, ours, theirs, &mut rescued, &mut dropped, &mut conflicts)
            }
            (Some(before), Some(mine), None) => {
                removed_or_moved(id, before, mine, theirs, ours, &mut rescued, &mut dropped, &mut conflicts)
            }
            (before, mine, other) => merge_object(before, mine, other, &mut conflicts),
        };
        if let Some(object) = resolved {
            kept.insert(id, object);
        }
    }
    for (id, survivor) in dropped {
        let mut seen = HashSet::new();
        let mut cursor = survivor.object(id).and_then(|object| object.parent);
        while let Some(parent) = cursor {
            if !seen.insert(parent.object) {
                break;
            }
            if rescued.contains(&parent.object) {
                if let Some(object) = survivor.object(id) {
                    kept.insert(id, object.clone());
                }
                break;
            }
            cursor = survivor.object(parent.object).and_then(|object| object.parent);
        }
    }
    if !kept.contains_key(&ObjectId::ROOT)
        && let Some(root) = ours.object(ObjectId::ROOT)
    {
        kept.insert(ObjectId::ROOT, root.clone());
    }
    rehome(&mut kept, base, [ours, theirs, base], &mut conflicts);
    restore_ancestors(&mut kept, [ours, theirs, base], &mut conflicts);
    let mut gone = base.gone().clone();
    gone.extend(theirs.gone().iter().map(|(id, left)| (*id, *left)));
    gone.extend(ours.gone().iter().map(|(id, left)| (*id, *left)));
    let mut merged = Tree::from_parts(kept, gone);
    rebuild_lists(&mut merged, base, ours, theirs, &mut conflicts);
    (merged, conflicts)
}

#[allow(clippy::too_many_arguments)]
fn removed_or_moved<'a>(
    id: ObjectId,
    before: &Object,
    survivor: &Object,
    remover: &Tree,
    keeper: &'a Tree,
    rescued: &mut BTreeSet<ObjectId>,
    dropped: &mut Vec<(ObjectId, &'a Tree)>,
    conflicts: &mut usize,
) -> Option<Object> {
    if same_content(before, survivor) {
        dropped.push((id, keeper));
        return None;
    }
    *conflicts += 1;
    let moved_out = survivor.parent != before.parent
        && before
            .parent
            .is_some_and(|parent| !remover.contains(parent.object));
    if moved_out {
        rescued.insert(id);
        return Some(survivor.clone());
    }
    dropped.push((id, keeper));
    None
}

fn same_content(before: &Object, after: &Object) -> bool {
    before.parent == after.parent
        && before.fields.len() == after.fields.len()
        && before
            .fields
            .iter()
            .zip(&after.fields)
            .all(|(before, after)| match (before, after) {
                (Value::List(_), Value::List(_)) => true,
                _ => before == after,
            })
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
        (None, Some(ours), Some(theirs)) => {
            Some(merge_fields(&blank_like(ours), ours, theirs, conflicts))
        }
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
                (Value::Grid(before), Value::Grid(mine), Value::Grid(other)) => {
                    Value::Grid(crate::Cells::merge(before, mine, other, conflicts))
                }
                _ => mine.clone(),
            }
        })
        .collect();
    Object { parent, fields }
}

fn blank_like(object: &Object) -> Object {
    Object {
        parent: None,
        fields: object
            .fields
            .iter()
            .map(|value| match value {
                Value::Register(_) => Value::Register(Vec::new()),
                Value::Count(_) => Value::Count(0),
                Value::List(_) => Value::List(Vec::new()),
                Value::Map(_) => Value::Map(BTreeMap::new()),
                Value::Grid(cells) => Value::Grid(cells.emptied()),
            })
            .collect(),
    }
}

fn merge_entries(
    base: &BTreeMap<Vec<u8>, Vec<u8>>,
    ours: &BTreeMap<Vec<u8>, Vec<u8>>,
    theirs: &BTreeMap<Vec<u8>, Vec<u8>>,
    conflicts: &mut usize,
) -> BTreeMap<Vec<u8>, Vec<u8>> {
    let keys: BTreeSet<&Vec<u8>> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    keys.into_iter()
        .filter_map(|key| {
            let (before, mine, other) = (base.get(key), ours.get(key), theirs.get(key));
            let picked = match (before, mine, other) {
                (Some(_), None, Some(changed)) | (Some(_), Some(changed), None)
                    if before != Some(changed) =>
                {
                    *conflicts += 1;
                    None
                }
                _ => pick(&before, &mine, &other, conflicts),
            };
            picked.map(|value| (key.clone(), value.clone()))
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

fn rehome(kept: &mut Objects, base: &Tree, versions: [&Tree; 3], conflicts: &mut usize) {
    let ids: Vec<ObjectId> = kept.keys().copied().collect();
    for id in ids {
        let Some(parent) = kept.get(&id).and_then(|object| object.parent) else {
            continue;
        };
        if kept.contains_key(&parent.object) {
            continue;
        }
        let Some(before) = base.object(id) else {
            continue;
        };
        let home = before
            .parent
            .filter(|home| holds_list(kept, *home))
            .or_else(|| sibling_list(kept, versions, parent));
        if let (Some(home), Some(object)) = (home, kept.get_mut(&id)) {
            object.parent = Some(home);
            *conflicts += 1;
        }
    }
}

fn holds_list(kept: &Objects, place: Place) -> bool {
    kept.get(&place.object)
        .and_then(|object| object.fields.get(usize::from(place.field)))
        .is_some_and(|value| matches!(value, Value::List(_)))
}

fn sibling_list(kept: &Objects, versions: [&Tree; 3], removed: Place) -> Option<Place> {
    let outer = versions
        .iter()
        .find_map(|version| version.object(removed.object))
        .and_then(|object| object.parent)?;
    let Some(Value::List(siblings)) = kept
        .get(&outer.object)?
        .fields
        .get(usize::from(outer.field))
    else {
        return None;
    };
    siblings
        .iter()
        .map(|sibling| Place {
            object: *sibling,
            field: removed.field,
        })
        .find(|place| holds_list(kept, *place))
}

fn rebuild_lists(merged: &mut Tree, base: &Tree, ours: &Tree, theirs: &Tree, conflicts: &mut usize) {
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
        let (before, mine, other) = (
            base.list_ids(place),
            ours.list_ids(place),
            theirs.list_ids(place),
        );
        let order = merge_order(&before, &mine, &other, conflicts);
        let belongs: BTreeSet<ObjectId> = children
            .get(&place)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default();
        let mut placed = BTreeSet::new();
        let mut items: Vec<ObjectId> = order
            .into_iter()
            .filter(|id| belongs.contains(id) && placed.insert(*id))
            .collect();
        let missing: Vec<ObjectId> = mine
            .iter()
            .chain(&other)
            .chain(&before)
            .chain(&belongs)
            .filter(|id| belongs.contains(id) && placed.insert(**id))
            .copied()
            .collect();
        for id in missing {
            let seen = [&mine, &other, &before]
                .into_iter()
                .find(|version| version.contains(&id));
            let index = seen.map_or(items.len(), |version| after_placed(&items, version, id, |_| true));
            items.insert(index, id);
        }
        merged.set_list(place, items);
    }
}

fn merge_order(
    before: &[ObjectId],
    mine: &[ObjectId],
    other: &[ObjectId],
    conflicts: &mut usize,
) -> Vec<ObjectId> {
    let moved_mine = moved(before, mine);
    let moved_other = moved(before, other);
    let still = |ids: &[ObjectId]| -> Vec<ObjectId> {
        ids.iter()
            .filter(|id| !moved_mine.contains(id) && !moved_other.contains(id))
            .copied()
            .collect()
    };
    let outcome = merge_slices(&still(before), &still(mine), &still(other));
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
    for id in mine.iter().filter(|id| moved_mine.contains(id)) {
        if moved_other.contains(id) && predecessor(mine, *id) != predecessor(other, *id) {
            *conflicts += 1;
        }
        let index = after_placed(&order, mine, *id, |_| true);
        order.insert(index, *id);
    }
    for id in other
        .iter()
        .filter(|id| moved_other.contains(id) && !moved_mine.contains(id))
    {
        let index = after_placed(&order, other, *id, |sibling| !moved_mine.contains(sibling));
        order.insert(index, *id);
    }
    order
}

fn predecessor(items: &[ObjectId], id: ObjectId) -> Option<ObjectId> {
    let index = items.iter().position(|held| *held == id)?;
    index.checked_sub(1).map(|before| items[before])
}

fn after_placed(
    placed: &[ObjectId],
    version: &[ObjectId],
    id: ObjectId,
    usable: impl Fn(&ObjectId) -> bool,
) -> usize {
    let Some(at) = version.iter().position(|held| *held == id) else {
        return placed.len();
    };
    version[..at]
        .iter()
        .rev()
        .filter(|sibling| usable(sibling))
        .find_map(|sibling| placed.iter().position(|held| held == sibling))
        .map_or(0, |index| index + 1)
}

fn moved(before: &[ObjectId], after: &[ObjectId]) -> HashSet<ObjectId> {
    let positions: BTreeMap<ObjectId, usize> = before
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    let kept: Vec<(ObjectId, usize)> = after
        .iter()
        .filter_map(|id| positions.get(id).map(|index| (*id, *index)))
        .collect();
    let mut tails: Vec<usize> = Vec::new();
    let mut previous: Vec<Option<usize>> = vec![None; kept.len()];
    for (at, (_, index)) in kept.iter().enumerate() {
        let length = tails.partition_point(|tail| kept[*tail].1 < *index);
        if length > 0 {
            previous[at] = Some(tails[length - 1]);
        }
        if length == tails.len() {
            tails.push(at);
        } else {
            tails[length] = at;
        }
    }
    let mut stable = HashSet::new();
    let mut cursor = tails.last().copied();
    while let Some(at) = cursor {
        stable.insert(kept[at].0);
        cursor = previous[at];
    }
    kept.into_iter()
        .map(|(id, _)| id)
        .filter(|id| !stable.contains(id))
        .collect()
}
