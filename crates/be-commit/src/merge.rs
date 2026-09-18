use std::collections::{BTreeMap, BTreeSet};

use similar::{Algorithm, DiffOp, capture_diff_slices};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict<T> {
    pub at: usize,
    pub base: Vec<T>,
    pub ours: Vec<T>,
    pub theirs: Vec<T>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeOutcome<T> {
    pub merged: Vec<T>,
    pub conflicts: Vec<Conflict<T>>,
}

impl<T> MergeOutcome<T> {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergeResult<T> {
    Clean(T),
    Conflicted { value: T, conflicts: usize },
}

impl<T> MergeResult<T> {
    pub fn value(self) -> T {
        match self {
            Self::Clean(value) | Self::Conflicted { value, .. } => value,
        }
    }

    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Clean(_))
    }
}

pub trait Merge: Sized {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self>;
}

fn base_positions<T: Eq + std::hash::Hash + Ord>(base: &[T], side: &[T]) -> Vec<Option<usize>> {
    let mut positions = vec![None; base.len()];
    for operation in capture_diff_slices(Algorithm::Myers, base, side) {
        if let DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = operation
        {
            for offset in 0..len {
                positions[old_index + offset] = Some(new_index + offset);
            }
        }
    }
    positions
}

pub fn merge_slices<T: Clone + Eq + std::hash::Hash + Ord>(
    base: &[T],
    ours: &[T],
    theirs: &[T],
) -> MergeOutcome<T> {
    let ours_positions = base_positions(base, ours);
    let theirs_positions = base_positions(base, theirs);

    let mut merged = Vec::new();
    let mut conflicts = Vec::new();
    let (mut base_cursor, mut ours_cursor, mut theirs_cursor) = (0usize, 0usize, 0usize);

    let anchors = (0..base.len())
        .filter_map(|index| Some((index, ours_positions[index]?, theirs_positions[index]?)));

    for (base_index, ours_index, theirs_index) in
        anchors.chain([(base.len(), ours.len(), theirs.len())])
    {
        if base_index > base_cursor || ours_index > ours_cursor || theirs_index > theirs_cursor {
            let base_region = &base[base_cursor..base_index];
            let ours_region = &ours[ours_cursor..ours_index];
            let theirs_region = &theirs[theirs_cursor..theirs_index];
            if ours_region == base_region {
                merged.extend_from_slice(theirs_region);
            } else if theirs_region == base_region || theirs_region == ours_region {
                merged.extend_from_slice(ours_region);
            } else {
                conflicts.push(Conflict {
                    at: merged.len(),
                    base: base_region.to_vec(),
                    ours: ours_region.to_vec(),
                    theirs: theirs_region.to_vec(),
                });
                merged.extend_from_slice(ours_region);
            }
        }
        if base_index < base.len() {
            merged.push(base[base_index].clone());
        }
        base_cursor = base_index + 1;
        ours_cursor = ours_index + 1;
        theirs_cursor = theirs_index + 1;
    }

    MergeOutcome { merged, conflicts }
}

pub fn split_lines(data: &[u8]) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in data.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(data[start..=index].to_vec());
            start = index + 1;
        }
    }
    if start < data.len() {
        lines.push(data[start..].to_vec());
    }
    lines
}

pub fn join_lines(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut data = Vec::new();
    for line in lines {
        data.extend_from_slice(line);
    }
    data
}

pub fn merge_lines(base: &[u8], ours: &[u8], theirs: &[u8]) -> MergeOutcome<Vec<u8>> {
    merge_slices(&split_lines(base), &split_lines(ours), &split_lines(theirs))
}

pub fn render_conflicts(
    outcome: &MergeOutcome<Vec<u8>>,
    ours_label: &str,
    theirs_label: &str,
) -> Vec<u8> {
    let mut rendered = Vec::new();
    let mut cursor = 0;
    for conflict in &outcome.conflicts {
        rendered.extend_from_slice(&join_lines(&outcome.merged[cursor..conflict.at]));
        rendered.extend_from_slice(format!("<<<<<<< {ours_label}\n").as_bytes());
        rendered.extend_from_slice(&join_lines(&conflict.ours));
        if !rendered.ends_with(b"\n") {
            rendered.push(b'\n');
        }
        rendered.extend_from_slice(b"=======\n");
        rendered.extend_from_slice(&join_lines(&conflict.theirs));
        if !rendered.ends_with(b"\n") {
            rendered.push(b'\n');
        }
        rendered.extend_from_slice(format!(">>>>>>> {theirs_label}\n").as_bytes());
        cursor = conflict.at + conflict.ours.len();
    }
    rendered.extend_from_slice(&join_lines(&outcome.merged[cursor..]));
    rendered
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MapMerge<K, V> {
    pub merged: BTreeMap<K, V>,
    pub conflicts: Vec<K>,
}

pub fn merge_map<K: Clone + Ord, V: Clone + Eq>(
    base: &BTreeMap<K, V>,
    ours: &BTreeMap<K, V>,
    theirs: &BTreeMap<K, V>,
) -> MapMerge<K, V> {
    let keys: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    let mut merged = BTreeMap::new();
    let mut conflicts = Vec::new();
    for key in keys {
        let in_base = base.get(key);
        let in_ours = ours.get(key);
        let in_theirs = theirs.get(key);
        let resolved = if in_ours == in_base {
            in_theirs.cloned()
        } else if in_theirs == in_base || in_theirs == in_ours {
            in_ours.cloned()
        } else {
            conflicts.push(key.clone());
            in_ours.cloned()
        };
        if let Some(value) = resolved {
            merged.insert(key.clone(), value);
        }
    }
    MapMerge { merged, conflicts }
}
