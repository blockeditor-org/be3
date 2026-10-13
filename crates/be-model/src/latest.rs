use std::{collections::BTreeMap, marker::PhantomData, ops::Deref};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::stored::{decode, encode};
use crate::{Change, Field, FieldRef, Object, ObjectId, Shape, Tree, Value};

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct Stamp {
    pub time: u64,
    pub origin: Uuid,
}

impl Stamp {
    pub fn after(self, time: u64, origin: Uuid) -> Self {
        Self {
            time: time.max(self.time.saturating_add(1)),
            origin,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Stamped {
    pub stamp: Stamp,
    pub value: Option<Vec<u8>>,
}

pub(crate) fn stamp(
    entries: &mut BTreeMap<Vec<u8>, Stamped>,
    key: &[u8],
    stamped: &Stamped,
) -> bool {
    if entries
        .get(key)
        .is_some_and(|held| held.stamp >= stamped.stamp)
    {
        return false;
    }
    entries.insert(key.to_vec(), stamped.clone());
    true
}

pub(crate) fn merge(
    ours: &BTreeMap<Vec<u8>, Stamped>,
    theirs: &BTreeMap<Vec<u8>, Stamped>,
) -> BTreeMap<Vec<u8>, Stamped> {
    let mut merged = ours.clone();
    for (key, stamped) in theirs {
        stamp(&mut merged, key, stamped);
    }
    merged
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Latest<T> {
    stamp: Stamp,
    value: T,
}

impl<T> Latest<T> {
    pub const fn stamp(&self) -> Stamp {
        self.stamp
    }

    pub fn next(&self, time: u64, origin: Uuid) -> Stamp {
        self.stamp.after(time, origin)
    }
}

impl<T> Deref for Latest<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: Serialize + DeserializeOwned + Default> Field for Latest<T> {
    fn blank() -> Value {
        Value::Latest(BTreeMap::new())
    }

    fn shape() -> Shape {
        Shape::Latest
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        let Value::Latest(entries) = value else {
            return Self {
                stamp: Stamp::default(),
                value: T::default(),
            };
        };
        let held = entries.get([].as_slice());
        Self {
            stamp: held.map(|held| held.stamp).unwrap_or_default(),
            value: held
                .and_then(|held| held.value.as_deref())
                .and_then(decode)
                .unwrap_or_default(),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Latest(
            [(
                Vec::new(),
                Stamped {
                    stamp: self.stamp,
                    value: Some(encode(&self.value)),
                },
            )]
            .into_iter()
            .collect(),
        )
    }
}

impl<M, T: Serialize> FieldRef<M, Latest<T>> {
    pub fn set(self, object: ObjectId, value: &T, stamp: Stamp) -> Change {
        Change::Stamp {
            object,
            field: self.index(),
            key: Vec::new(),
            stamped: Stamped {
                stamp,
                value: Some(encode(value)),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatestMap<K, V> {
    entries: BTreeMap<K, V>,
    stamps: BTreeMap<K, Stamp>,
    marker: PhantomData<fn() -> V>,
}

impl<K, V> Default for LatestMap<K, V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            stamps: BTreeMap::new(),
            marker: PhantomData,
        }
    }
}

impl<K: Ord, V> LatestMap<K, V> {
    pub fn stamp(&self, key: &K) -> Stamp {
        self.stamps.get(key).copied().unwrap_or_default()
    }

    pub fn next(&self, key: &K, time: u64, origin: Uuid) -> Stamp {
        self.stamp(key).after(time, origin)
    }
}

impl<K, V> Deref for LatestMap<K, V> {
    type Target = BTreeMap<K, V>;

    fn deref(&self) -> &BTreeMap<K, V> {
        &self.entries
    }
}

impl<K: Serialize + DeserializeOwned + Ord + Clone, V: Serialize + DeserializeOwned> Field
    for LatestMap<K, V>
{
    fn blank() -> Value {
        Value::Latest(BTreeMap::new())
    }

    fn shape() -> Shape {
        Shape::Latest
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        let Value::Latest(held) = value else {
            return Self::default();
        };
        let mut map = Self::default();
        for (key, stamped) in held {
            let Some(key) = decode::<K>(key) else {
                continue;
            };
            map.stamps.insert(key.clone(), stamped.stamp);
            if let Some(value) = stamped.value.as_deref().and_then(decode) {
                map.entries.insert(key, value);
            }
        }
        map
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Latest(
            self.stamps
                .iter()
                .map(|(key, stamp)| {
                    (
                        encode(key),
                        Stamped {
                            stamp: *stamp,
                            value: self.entries.get(key).map(encode),
                        },
                    )
                })
                .collect(),
        )
    }
}

impl<M, K: Serialize, V: Serialize> FieldRef<M, LatestMap<K, V>> {
    pub fn put(self, object: ObjectId, key: &K, value: Option<&V>, stamp: Stamp) -> Change {
        Change::Stamp {
            object,
            field: self.index(),
            key: encode(key),
            stamped: Stamped {
                stamp,
                value: value.map(encode),
            },
        }
    }
}
