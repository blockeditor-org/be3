use std::collections::{BTreeMap, HashMap};

use crate::Anchor;

#[derive(Clone, Debug, Default)]
pub struct AnchorTable {
    by_index: BTreeMap<usize, Anchor>,
    by_anchor: HashMap<Anchor, usize>,
}

impl AnchorTable {
    pub fn anchor(&mut self, index: usize) -> Anchor {
        if let Some(anchor) = self.by_index.get(&index) {
            return *anchor;
        }
        let anchor = Anchor::new();
        self.insert(index, anchor);
        anchor
    }

    pub fn index(&self, anchor: Anchor) -> Option<usize> {
        self.by_anchor.get(&anchor).copied()
    }

    pub fn len(&self) -> usize {
        self.by_index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_index.is_empty()
    }

    fn insert(&mut self, index: usize, anchor: Anchor) {
        if let Some(replaced) = self.by_index.insert(index, anchor) {
            self.by_anchor.remove(&replaced);
        }
        self.by_anchor.insert(anchor, index);
    }

    pub fn splice(&mut self, index: usize, delete: usize, insert: usize) -> Vec<(usize, Anchor)> {
        let mut tail = self.by_index.split_off(&index);
        let after = tail.split_off(&(index + delete));
        let removed = tail
            .into_iter()
            .map(|(held, anchor)| {
                self.by_anchor.remove(&anchor);
                (held - index, anchor)
            })
            .collect();
        for (held, anchor) in after {
            self.insert(held - delete + insert, anchor);
        }
        removed
    }

    pub fn restore(&mut self, index: usize, anchors: &[(usize, Anchor)]) {
        for (offset, anchor) in anchors {
            if !self.by_index.contains_key(&(index + offset)) {
                self.insert(index + offset, *anchor);
            }
        }
    }

    pub fn remap(&mut self, mut moved: impl FnMut(usize) -> Option<usize>) {
        let held = std::mem::take(&mut self.by_index);
        self.by_anchor.clear();
        for (index, anchor) in held {
            if let Some(index) = moved(index) {
                self.insert(index, anchor);
            }
        }
    }
}
