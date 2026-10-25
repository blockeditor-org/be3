use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::Deref,
};

use ciborium::tag::Required;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{ObjectId, Stored, Tree, Value, stored};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockRef(pub Uuid);

impl Deref for BlockRef {
    type Target = Uuid;

    fn deref(&self) -> &Uuid {
        &self.0
    }
}

impl From<Uuid> for BlockRef {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Serialize for BlockRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        block_ref::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for BlockRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        block_ref::deserialize(deserializer).map(Self)
    }
}

pub mod block_ref {
    use super::*;

    pub fn serialize<S: Serializer>(id: &Uuid, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            return id.serialize(serializer);
        }
        Required::<&Uuid, { stored::BLOCK_REF }>(id).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Uuid, D::Error> {
        if deserializer.is_human_readable() {
            return Uuid::deserialize(deserializer);
        }
        Required::<Uuid, { stored::BLOCK_REF }>::deserialize(deserializer).map(|held| held.0)
    }

    pub mod option {
        use super::*;

        pub fn serialize<S: Serializer>(
            id: &Option<Uuid>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            id.map(BlockRef).serialize(serializer)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Uuid>, D::Error> {
            Option::<BlockRef>::deserialize(deserializer).map(|held| held.map(|id| id.0))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Index {
    held: HashMap<ObjectId, BTreeMap<u16, Vec<Uuid>>>,
    counts: HashMap<Uuid, usize>,
    order: Vec<Uuid>,
}

const KEPT: u16 = u16::MAX;

impl Index {
    pub(crate) fn build(tree: &Tree) -> Self {
        let mut index = Self::default();
        let mut found = Vec::new();
        index.visit(tree, ObjectId::ROOT, &mut found);
        let mut seen = HashSet::new();
        index.order = found.into_iter().filter(|id| seen.insert(*id)).collect();
        index
    }

    fn visit(&mut self, tree: &Tree, id: ObjectId, found: &mut Vec<Uuid>) {
        let Some(object) = tree.object(id) else {
            return;
        };
        let mut slots = BTreeMap::new();
        if object.kept.kind.is_none() {
            for (slot, value) in object.fields.iter().enumerate() {
                match value {
                    Value::List(items) => {
                        for child in items.iter() {
                            self.visit(tree, *child, found);
                        }
                    }
                    value => {
                        let mut own = Vec::new();
                        walk_value(value, &mut own);
                        found.extend_from_slice(&own);
                        if let Ok(slot) = u16::try_from(slot) {
                            slots.insert(slot, own);
                        }
                    }
                }
            }
        }
        let mut kept = Vec::new();
        for held in object.kept.properties.values() {
            walk_bytes(held, &mut kept);
        }
        found.extend_from_slice(&kept);
        slots.insert(KEPT, kept);
        for (slot, own) in slots {
            let own = distinct(own);
            for held in &own {
                *self.counts.entry(*held).or_default() += 1;
            }
            if !own.is_empty() {
                self.held.entry(id).or_default().insert(slot, own);
            }
        }
    }

    pub(crate) fn ids(&self) -> Vec<Uuid> {
        self.order.clone()
    }

    pub(crate) fn refresh(&mut self, tree: &Tree, id: ObjectId) {
        let slots: Vec<u16> = self
            .held
            .get(&id)
            .map(|slots| slots.keys().copied().collect())
            .unwrap_or_default();
        for slot in slots {
            self.set(id, slot, Vec::new());
        }
        let Some(object) = tree.object(id) else {
            return;
        };
        let count = u16::try_from(object.fields.len()).unwrap_or(KEPT);
        for slot in 0..count {
            self.refresh_field(tree, id, slot);
        }
        let mut kept = Vec::new();
        for held in object.kept.properties.values() {
            walk_bytes(held, &mut kept);
        }
        self.set(id, KEPT, kept);
    }

    pub(crate) fn refresh_field(&mut self, tree: &Tree, id: ObjectId, field: u16) {
        let mut found = Vec::new();
        if let Some(object) = tree.object(id)
            && object.kept.kind.is_none()
            && let Some(value) = object.fields.get(usize::from(field))
        {
            walk_value(value, &mut found);
        }
        self.set(id, field, found);
    }

    pub(crate) fn add(&mut self, tree: &Tree, id: ObjectId, field: u16, items: &[Vec<u8>]) {
        if tree
            .object(id)
            .is_none_or(|object| object.kept.kind.is_some())
        {
            return;
        }
        let mut found = Vec::new();
        for item in items {
            walk_bytes(item, &mut found);
        }
        if found.is_empty() {
            return;
        }
        let mut held = self
            .held
            .get(&id)
            .and_then(|slots| slots.get(&field))
            .cloned()
            .unwrap_or_default();
        held.extend(found);
        self.set(id, field, held);
    }

    fn set(&mut self, id: ObjectId, slot: u16, found: Vec<Uuid>) {
        let gone = self
            .held
            .get_mut(&id)
            .and_then(|slots| slots.remove(&slot))
            .unwrap_or_default();
        if self.held.get(&id).is_some_and(BTreeMap::is_empty) {
            self.held.remove(&id);
        }
        if found.is_empty() && gone.is_empty() {
            return;
        }
        for gone in gone {
            let Some(count) = self.counts.get_mut(&gone) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.counts.remove(&gone);
                self.order.retain(|held| *held != gone);
            }
        }
        let found = distinct(found);
        for added in &found {
            let count = self.counts.entry(*added).or_default();
            if *count == 0 {
                self.order.push(*added);
            }
            *count += 1;
        }
        if !found.is_empty() {
            self.held.entry(id).or_default().insert(slot, found);
        }
    }
}

fn distinct(found: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    found.into_iter().filter(|id| seen.insert(*id)).collect()
}

fn walk_value(value: &Value, found: &mut Vec<Uuid>) {
    match value {
        Value::Register(held) => walk_bytes(held, found),
        Value::Map(entries) => {
            for (key, value) in entries {
                walk_bytes(key, found);
                walk_bytes(value, found);
            }
        }
        Value::Latest(entries) => {
            for (key, stamped) in entries {
                walk_bytes(key, found);
                if let Some(value) = &stamped.value {
                    walk_bytes(value, found);
                }
            }
        }
        Value::Series(items) => {
            for item in items.iter() {
                walk_bytes(item, found);
            }
        }
        Value::List(_) | Value::Count(_) | Value::Grid(_) | Value::Text(_) | Value::Rich(_) => {}
    }
}

fn walk_bytes(bytes: &[u8], found: &mut Vec<Uuid>) {
    let [_, _, _, _, a, b, c, d] = stored::BLOCK_REF.to_be_bytes();
    let tagged = [0xda, a, b, c, d];
    if !bytes.windows(tagged.len()).any(|window| window == tagged) {
        return;
    }
    if let Some(value) = stored::decode::<Stored>(bytes) {
        walk(&value, found);
    }
}

pub fn walk(value: &Stored, found: &mut Vec<Uuid>) {
    match value {
        Stored::Tag(stored::BLOCK_REF, inner) => {
            if let Some(id) = inner
                .as_bytes()
                .and_then(|bytes| Uuid::from_slice(bytes).ok())
            {
                found.push(id);
            }
        }
        Stored::Tag(_, inner) => walk(inner, found),
        Stored::Array(items) => {
            for item in items {
                walk(item, found);
            }
        }
        Stored::Map(entries) => {
            for (key, value) in entries {
                walk(key, found);
                walk(value, found);
            }
        }
        _ => {}
    }
}
