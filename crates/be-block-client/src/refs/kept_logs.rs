//! How a kept access log (`KeptLog`) is stored: a row of its own, holding what does not grow with
//! the log, and a row for each of its places (`KeptPlace`), holding what does: the entry's hash,
//! the step served there, and what the check noted at it. A log kept again as it grew writes its
//! own row and the places it added alone, so a log's appends write each entry once over its life
//! rather than the whole log on each (guides/blocks/keys.md, "Access logs").

use std::collections::BTreeMap;

use be_block_keys::{Level, Placement, Step};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeptLog, Row, Table, row};

/// What a kept log holds at one of its places.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct KeptPlace {
    /// The hash of the entry there.
    hash: [u8; 32],
    /// The step served there (`KeptLog::steps`), for a whole log.
    step: Option<Step>,
    /// The levels the log set there, in the order it set them (`Checked::levels`).
    levels: Vec<([u8; 16], Level)>,
    /// Where a parent record there placed the block (`Checked::placements`).
    placement: Option<Option<Placement>>,
}

/// The first place `kept` keeps a row for: every place of a log kept checked or with its steps,
/// and only those of `current` (from the last lowering) for a proof path, which keeps nothing else
/// by place.
fn first(kept: &KeptLog) -> u64 {
    match kept.checked.is_some() || !kept.steps.is_empty() {
        true => 0,
        false => kept.lowering_place,
    }
}

/// Whether places of `old` and `new` are kept alike, so a place both keep is the same row.
fn alike(old: &KeptLog, new: &KeptLog) -> bool {
    old.checked.is_some() == new.checked.is_some() && old.steps.is_empty() == new.steps.is_empty()
}

fn key(block: Uuid, place: u64) -> Vec<u8> {
    let mut key = block.as_bytes().to_vec();
    key.extend(place.to_be_bytes());
    key
}

/// The hash `kept` holds at `place`, if it holds one.
fn hash_at(kept: &KeptLog, place: u64) -> Option<[u8; 32]> {
    let at = usize::try_from(place).ok()?;
    if let Some(step) = kept.steps.get(at) {
        return Some(step.hash());
    }
    if let Some(hash) = place
        .checked_sub(kept.lowering_place)
        .and_then(|since| kept.current.get(usize::try_from(since).ok()?))
    {
        return Some(*hash);
    }
    kept.checked.as_ref().and_then(|checked| {
        checked
            .positions
            .iter()
            .find(|(_, held)| **held == at)
            .map(|(hash, _)| *hash)
    })
}

/// `kept`'s places from `from` through its head, as rows.
fn places(block: Uuid, kept: &KeptLog, from: u64) -> Vec<Row> {
    let mut places: BTreeMap<u64, KeptPlace> = BTreeMap::new();
    for place in from..=kept.place {
        let Some(hash) = hash_at(kept, place) else {
            continue;
        };
        let step = usize::try_from(place)
            .ok()
            .and_then(|at| kept.steps.get(at))
            .cloned();
        places.insert(
            place,
            KeptPlace {
                hash,
                step,
                levels: Vec::new(),
                placement: None,
            },
        );
    }
    if let Some(checked) = &kept.checked {
        let mut accounts: Vec<_> = checked.levels.iter().collect();
        accounts.sort_by_key(|(account, _)| **account);
        for (account, changes) in accounts {
            for (at, level) in changes {
                if let Some(place) = places.get_mut(&(*at as u64)) {
                    place.levels.push((*account, *level));
                }
            }
        }
        for (at, placement) in &checked.placements {
            if let Some(place) = places.get_mut(&(*at as u64)) {
                place.placement = Some(*placement);
            }
        }
    }
    places
        .into_iter()
        .map(|(place, held)| row(Table::LogPlace, key(block, place), &held))
        .collect()
}

/// `kept` less what its places hold, as its own row keeps it.
fn own(kept: &KeptLog) -> KeptLog {
    KeptLog {
        current: Vec::new(),
        steps: Vec::new(),
        checked: kept.checked.as_ref().map(|checked| {
            let mut own = checked.clone();
            own.positions.clear();
            own.levels.clear();
            own.placements.clear();
            own
        }),
        ..kept.clone()
    }
}

fn deleted(block: Uuid, place: u64) -> Row {
    Row {
        table: Table::LogPlace,
        key: key(block, place),
        value: None,
    }
}

/// The rows that keep `new` as `block`'s log where `old` was kept. A log that grew from `old`,
/// reaching its head at its place, writes the places after it alone (and drops those a proof
/// path's newer lowering no longer keeps); any other writes every place it keeps, and drops what
/// `old` kept beyond them.
pub(super) fn rows(block: Uuid, old: Option<&KeptLog>, new: &KeptLog) -> Vec<Row> {
    let from = first(new);
    let mut rows = vec![row(Table::Logs, block.as_bytes().to_vec(), &own(new))];
    let grew = old.filter(|old| {
        alike(old, new)
            && new.place >= old.place
            && from >= first(old)
            && hash_at(new, old.place) == Some(old.head)
    });
    match grew {
        Some(old) => {
            rows.extend((first(old)..from.min(old.place + 1)).map(|place| deleted(block, place)));
            rows.extend(places(block, new, from.max(old.place + 1)));
        }
        None => {
            if let Some(old) = old {
                rows.extend(
                    (first(old)..=old.place)
                        .filter(|place| !(from..=new.place).contains(place))
                        .map(|place| deleted(block, place)),
                );
            }
            rows.extend(places(block, new, from));
        }
    }
    rows
}

/// The rows that drop `kept`, `block`'s log, whole.
pub(super) fn removed(block: Uuid, kept: &KeptLog) -> Vec<Row> {
    let mut rows = vec![Row {
        table: Table::Logs,
        key: block.as_bytes().to_vec(),
        value: None,
    }];
    rows.extend((first(kept)..=kept.place).map(|place| deleted(block, place)));
    rows
}

/// Joins `kept`, as its own row read back, with `places`, its places read back: the log as it
/// was kept. Answers false, joining nothing, when a place it keeps is missing (a write lost, or a
/// row the database dropped), or holds a step where another holds none: a log joined from part
/// of its places would be taken for one this device checked, so it is dropped instead and
/// checked afresh. Places past its head are left out.
pub(super) fn join(kept: &mut KeptLog, mut places: BTreeMap<u64, KeptPlace>) -> bool {
    places.retain(|place, _| *place <= kept.place);
    let stepped = places.values().filter(|place| place.step.is_some()).count();
    let from = match kept.checked.is_some() || stepped > 0 {
        true => 0,
        false => kept.lowering_place,
    };
    places.retain(|place, _| *place >= from);
    let whole = (from..=kept.place).all(|place| places.contains_key(&place));
    if !whole || (stepped > 0 && stepped != places.len()) {
        return false;
    }
    kept.current = (kept.lowering_place..=kept.place)
        .filter_map(|place| places.get(&place).map(|place| place.hash))
        .collect();
    kept.steps = places
        .values()
        .filter_map(|place| place.step.clone())
        .collect();
    if let Some(checked) = &mut kept.checked {
        for (place, held) in &places {
            let at = *place as usize;
            checked.positions.insert(held.hash, at);
            for (account, level) in &held.levels {
                checked
                    .levels
                    .entry(*account)
                    .or_default()
                    .push((at, *level));
            }
            if let Some(placement) = held.placement {
                checked.placements.push((at, placement));
            }
        }
    }
    true
}
