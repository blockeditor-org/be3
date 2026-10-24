use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use ciborium::tag::Required;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{Object, ObjectId, Stored, Tree, Value, stored};

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
    held: HashMap<ObjectId, Vec<Uuid>>,
    counts: HashMap<Uuid, usize>,
    order: Vec<Uuid>,
}

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
        let mut own = Vec::new();
        if object.kept.kind.is_none() {
            for value in &object.fields {
                match value {
                    Value::List(items) => {
                        for child in items.iter() {
                            self.visit(tree, *child, found);
                        }
                    }
                    value => {
                        let start = own.len();
                        walk_value(value, &mut own);
                        found.extend_from_slice(&own[start..]);
                    }
                }
            }
        }
        let start = own.len();
        for held in object.kept.properties.values() {
            walk_bytes(held, &mut own);
        }
        found.extend_from_slice(&own[start..]);
        let mut seen = HashSet::new();
        own.retain(|held| seen.insert(*held));
        for held in &own {
            *self.counts.entry(*held).or_default() += 1;
        }
        if !own.is_empty() {
            self.held.insert(id, own);
        }
    }

    pub(crate) fn ids(&self) -> Vec<Uuid> {
        self.order.clone()
    }

    pub(crate) fn refresh(&mut self, tree: &Tree, id: ObjectId) {
        let found = tree.object(id).map(held_by).unwrap_or_default();
        self.set(id, found);
    }

    fn set(&mut self, id: ObjectId, found: Vec<Uuid>) {
        if found.is_empty() && !self.held.contains_key(&id) {
            return;
        }
        for gone in self.held.remove(&id).unwrap_or_default() {
            let Some(count) = self.counts.get_mut(&gone) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.counts.remove(&gone);
                self.order.retain(|held| *held != gone);
            }
        }
        let mut seen = HashSet::new();
        let found: Vec<Uuid> = found.into_iter().filter(|id| seen.insert(*id)).collect();
        for added in &found {
            let count = self.counts.entry(*added).or_default();
            if *count == 0 {
                self.order.push(*added);
            }
            *count += 1;
        }
        if !found.is_empty() {
            self.held.insert(id, found);
        }
    }
}

fn held_by(object: &Object) -> Vec<Uuid> {
    let mut found = Vec::new();
    if object.kept.kind.is_none() {
        for value in &object.fields {
            walk_value(value, &mut found);
        }
    }
    for held in object.kept.properties.values() {
        walk_bytes(held, &mut found);
    }
    found
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
