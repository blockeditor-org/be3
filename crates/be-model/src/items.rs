use sequence::{Malformed, State};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Anchor, ObjectId, Pos, SeqOp, Sequence, Span};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Items(Sequence<ObjectId>);

pub(crate) fn pos(id: ObjectId) -> Pos {
    Pos {
        client: id.as_uuid().as_u64_pair().1,
        offset: 0,
    }
}

fn span(id: ObjectId) -> Span {
    let at = pos(id);
    Span {
        client: at.client,
        start: at.offset,
        len: 1,
    }
}

impl Items {
    pub fn from_ids(ids: impl IntoIterator<Item = ObjectId>) -> Self {
        Self(Sequence::from_placed(
            ids.into_iter().map(|id| (pos(id), id)).collect(),
        ))
    }

    pub fn ids(&self) -> Vec<ObjectId> {
        self.0.items()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ObjectId> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn index_of(&self, id: ObjectId) -> Option<usize> {
        match self.0.place_of(pos(id))? {
            (index, true) => (self.0.get(index) == Some(&id)).then_some(index),
            (_, false) => None,
        }
    }

    pub(crate) fn admit(&self, id: ObjectId, anchor: Anchor) -> Option<Option<Pos>> {
        if matches!(self.0.place_of(pos(id)), Some((_, true))) {
            return None;
        }
        let after = self.resolve(anchor);
        (after != Some(pos(id))).then_some(after)
    }

    pub(crate) fn is_fresh(&self) -> bool {
        self.0.fragment_count() == self.0.len()
    }

    pub(crate) fn refreshed(&self) -> Self {
        Self::from_ids(self.ids())
    }

    pub(crate) fn state(&self) -> State {
        self.0.state()
    }

    pub(crate) fn from_state(state: State, visible: &[ObjectId]) -> Result<Self, Malformed> {
        Sequence::from_state(state, visible).map(Self)
    }

    pub(crate) fn resolve(&self, anchor: Anchor) -> Option<Pos> {
        let behind = |at: Pos| match self.0.place_of(at) {
            Some(_) => Some(at),
            None => self.last(),
        };
        match anchor {
            Anchor::Start => None,
            Anchor::End => self.last(),
            Anchor::After(id) => behind(pos(id)),
            Anchor::Behind(at) => behind(at),
        }
    }

    fn last(&self) -> Option<Pos> {
        self.0
            .len()
            .checked_sub(1)
            .and_then(|index| self.0.pos(index))
    }

    pub(crate) fn anchor_of(&self, id: ObjectId) -> Anchor {
        match self.0.previous(pos(id)) {
            Some(Some(at)) => Anchor::Behind(at),
            Some(None) => Anchor::Start,
            None => Anchor::End,
        }
    }

    pub(crate) fn insert(&mut self, id: ObjectId, after: Option<Pos>) -> bool {
        let at = pos(id);
        if after == Some(at) {
            return false;
        }
        match self.0.place_of(at) {
            Some((_, true)) => false,
            Some((_, false)) => {
                let shown = self
                    .0
                    .apply(&SeqOp::Undelete {
                        spans: vec![span(id)],
                        items: vec![id],
                    })
                    .is_some();
                self.0.apply(&SeqOp::Move {
                    first: at,
                    last: at,
                    after,
                });
                shown
            }
            None => self
                .0
                .apply(&SeqOp::Insert {
                    after,
                    client: at.client,
                    start: at.offset,
                    items: vec![id],
                })
                .is_some(),
        }
    }

    pub(crate) fn remove(&mut self, id: ObjectId) -> bool {
        self.index_of(id).is_some()
            && self
                .0
                .apply(&SeqOp::Delete {
                    spans: vec![span(id)],
                })
                .is_some()
    }

    pub(crate) fn shift(&mut self, id: ObjectId, after: Option<Pos>) -> bool {
        let at = pos(id);
        if after == Some(at) || self.index_of(id).is_none() {
            return false;
        }
        let before = self.0.previous(at);
        self.0
            .apply(&SeqOp::Move {
                first: at,
                last: at,
                after,
            })
            .is_some()
            && self.0.previous(at) != before
    }

    pub(crate) fn stays(&self, id: ObjectId, after: Option<Pos>) -> bool {
        self.0.previous(pos(id)) == Some(after)
    }
}

impl Serialize for Items {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
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
