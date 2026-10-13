use std::{collections::HashSet, ops::Deref};

use ciborium::tag::Required;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{Kind, ObjectId, Shape, Stored, Tree, Value, stored};

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

pub(crate) fn collect(tree: &Tree, kind: &Kind) -> Vec<Uuid> {
    let mut found = Vec::new();
    walk_object(tree, ObjectId::ROOT, kind, &mut found);
    let mut seen = HashSet::new();
    found.retain(|id| seen.insert(*id));
    found
}

fn walk_object(tree: &Tree, id: ObjectId, kind: &Kind, found: &mut Vec<Uuid>) {
    let Some(object) = tree.object(id) else {
        return;
    };
    if object.kept.kind.is_none() {
        for (value, property) in object.fields.iter().zip(&kind.properties) {
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
                Value::List(items) => {
                    if let Shape::List(child) = &property.shape {
                        let child = child();
                        for item in items.ids() {
                            walk_object(tree, item, &child, found);
                        }
                    }
                }
                Value::Count(_) | Value::Grid(_) | Value::Text(_) => {}
            }
        }
    }
    for held in object.kept.properties.values() {
        walk_bytes(held, found);
    }
}

fn walk_bytes(bytes: &[u8], found: &mut Vec<Uuid>) {
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
