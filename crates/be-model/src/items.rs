use std::collections::BTreeMap;

use sequence::{Malformed, State};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Anchor, ObjectId, Pos, SeqOp, Sequence, Span};

#[derive(Clone, Debug, Default)]
pub struct Items {
    sequence: Sequence<ObjectId>,
    slots: BTreeMap<ObjectId, Pos>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct ItemsState {
    state: State,
    hidden: Vec<(ObjectId, Pos)>,
}

fn span(at: Pos) -> Span {
    Span {
        client: at.client,
        start: at.offset,
        len: 1,
    }
}

impl Items {
    pub fn from_ids(ids: impl IntoIterator<Item = ObjectId>) -> Self {
        let mut unique = Vec::new();
        let mut slots = BTreeMap::new();
        for id in ids {
            let at = Pos {
                client: sequence::LOADED,
                offset: unique.len() as u64,
            };
            if slots.insert(id, at).is_none() {
                unique.push(id);
            }
        }
        Self {
            sequence: Sequence::from_items(unique),
            slots,
        }
    }

    pub fn ids(&self) -> Vec<ObjectId> {
        self.sequence.items()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ObjectId> {
        self.sequence.iter()
    }

    pub fn len(&self) -> usize {
        self.sequence.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sequence.is_empty()
    }

    pub fn index_of(&self, id: ObjectId) -> Option<usize> {
        match self.sequence.place_of(*self.slots.get(&id)?)? {
            (index, true) => Some(index),
            (_, false) => None,
        }
    }

    pub(crate) fn is_fresh(&self) -> bool {
        self.sequence.is_fresh()
    }

    pub(crate) fn refreshed(&self) -> Self {
        Self::from_ids(self.ids())
    }

    pub(crate) fn state(&self) -> ItemsState {
        ItemsState {
            state: self.sequence.state(),
            hidden: self
                .slots
                .iter()
                .filter(|(_, at)| matches!(self.sequence.place_of(**at), Some((_, false))))
                .map(|(id, at)| (*id, *at))
                .collect(),
        }
    }

    pub(crate) fn from_state(held: ItemsState, visible: &[ObjectId]) -> Result<Self, Malformed> {
        let sequence = Sequence::from_state(held.state, visible)?;
        let mut slots = BTreeMap::new();
        for (index, id) in visible.iter().enumerate() {
            if slots
                .insert(*id, sequence.pos(index).ok_or(Malformed)?)
                .is_some()
            {
                return Err(Malformed);
            }
        }
        for (id, at) in held.hidden {
            if !matches!(sequence.place_of(at), Some((_, false))) || slots.contains_key(&id) {
                return Err(Malformed);
            }
            slots.insert(id, at);
        }
        Ok(Self { sequence, slots })
    }

    pub(crate) fn resolve(&self, anchor: Anchor) -> Option<Pos> {
        let behind = |at: Option<Pos>| match at.filter(|at| self.sequence.place_of(*at).is_some()) {
            Some(at) => Some(at),
            None => self.last(),
        };
        match anchor {
            Anchor::Start => None,
            Anchor::End => self.last(),
            Anchor::After(id) => behind(self.slots.get(&id).copied()),
            Anchor::Behind(at) => behind(Some(at)),
        }
    }

    fn last(&self) -> Option<Pos> {
        self.sequence
            .len()
            .checked_sub(1)
            .and_then(|index| self.sequence.pos(index))
    }

    pub(crate) fn anchor_of(&self, id: ObjectId) -> Anchor {
        match self
            .slots
            .get(&id)
            .and_then(|at| self.sequence.previous(*at))
        {
            Some(Some(at)) => Anchor::Behind(at),
            Some(None) => Anchor::Start,
            None => Anchor::End,
        }
    }

    pub(crate) fn admit(&self, id: ObjectId, anchor: Anchor) -> Option<Option<Pos>> {
        if self.index_of(id).is_some() {
            return None;
        }
        let after = self.resolve(anchor);
        (after.is_none() || after != self.slots.get(&id).copied()).then_some(after)
    }

    pub(crate) fn insert(&mut self, id: ObjectId, client: u64, after: Option<Pos>) -> bool {
        if self.index_of(id).is_some() {
            return false;
        }
        if let Some(at) = self.slots.get(&id).copied() {
            let shown = self
                .sequence
                .apply(&SeqOp::Undelete {
                    spans: vec![span(at)],
                    items: vec![id],
                })
                .is_some();
            if shown && after != Some(at) {
                self.sequence.apply(&SeqOp::Move {
                    first: at,
                    last: at,
                    after,
                });
            }
            return shown;
        }
        let at = Pos {
            client,
            offset: self.sequence.next_offset(client),
        };
        let placed = self
            .sequence
            .apply(&SeqOp::Insert {
                after,
                client: at.client,
                start: at.offset,
                items: vec![id],
            })
            .is_some();
        if placed {
            self.slots.insert(id, at);
        }
        placed
    }

    pub(crate) fn remove(&mut self, id: ObjectId) -> bool {
        let Some(at) = self.slots.get(&id).copied() else {
            return false;
        };
        self.index_of(id).is_some()
            && self
                .sequence
                .apply(&SeqOp::Delete {
                    spans: vec![span(at)],
                })
                .is_some()
    }

    pub(crate) fn shift(&mut self, id: ObjectId, after: Option<Pos>) -> bool {
        let Some(at) = self.slots.get(&id).copied() else {
            return false;
        };
        if after == Some(at) || self.index_of(id).is_none() {
            return false;
        }
        let before = self.sequence.previous(at);
        self.sequence
            .apply(&SeqOp::Move {
                first: at,
                last: at,
                after,
            })
            .is_some()
            && self.sequence.previous(at) != before
    }

    pub(crate) fn stays(&self, id: ObjectId, after: Option<Pos>) -> bool {
        self.slots
            .get(&id)
            .is_some_and(|at| self.sequence.previous(*at) == Some(after))
    }
}

impl PartialEq for Items {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}

impl Eq for Items {}

impl Serialize for Items {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.sequence.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Items {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<ObjectId>::deserialize(deserializer).map(Self::from_ids)
    }
}

impl FromIterator<ObjectId> for Items {
    fn from_iter<I: IntoIterator<Item = ObjectId>>(ids: I) -> Self {
        Self::from_ids(ids)
    }
}
