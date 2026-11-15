use std::{collections::BTreeMap, marker::PhantomData, ops::Deref};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::stored::{decode, encode};
use crate::{Change, Field, FieldRef, Object, ObjectId, Shape, Stamp, Tree, Value};

/// One key's value in a `RankedMap`, or none for one removed, with its rank and stamp.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ranked {
    pub rank: u64,
    pub stamp: Stamp,
    pub value: Option<Vec<u8>>,
}

impl Ranked {
    fn order(&self) -> (u64, Stamp) {
        (self.rank, self.stamp)
    }
}

/// Puts `ranked` at `key` unless what is held there ranks as high or higher: a lower rank never
/// replaces a higher one, live or in a merge.
pub(crate) fn rank(entries: &mut BTreeMap<Vec<u8>, Ranked>, key: &[u8], ranked: &Ranked) -> bool {
    if entries
        .get(key)
        .is_some_and(|held| held.order() >= ranked.order())
    {
        return false;
    }
    entries.insert(key.to_vec(), ranked.clone());
    true
}

/// Each key's highest: the order of merging never matters, and no side lowers a rank.
pub(crate) fn merge(
    ours: &BTreeMap<Vec<u8>, Ranked>,
    theirs: &BTreeMap<Vec<u8>, Ranked>,
) -> BTreeMap<Vec<u8>, Ranked> {
    let mut merged = ours.clone();
    for (key, ranked) in theirs {
        rank(&mut merged, key, ranked);
    }
    merged
}

/// A map whose keys each keep the value of the highest rank written, then the latest stamp
/// (`Ranked`): a rank never goes down, so a value is replaced only by one ranked as high or
/// higher. A value can be removed (none) at a rank like any other. `RankedMap<K, ()>` is a
/// map of numbers that only rise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedMap<K, V> {
    entries: BTreeMap<K, V>,
    ranks: BTreeMap<K, (u64, Stamp)>,
    marker: PhantomData<fn() -> V>,
}

impl<K, V> Default for RankedMap<K, V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            ranks: BTreeMap::new(),
            marker: PhantomData,
        }
    }
}

impl<K: Ord, V> RankedMap<K, V> {
    /// The rank held at `key`, 0 for one never written.
    pub fn rank(&self, key: &K) -> u64 {
        self.ranks.get(key).map_or(0, |(rank, _)| *rank)
    }

    /// The stamp for a write at `key` at `time` by `origin`, after the one held.
    pub fn next(&self, key: &K, time: u64, origin: uuid::Uuid) -> Stamp {
        self.ranks
            .get(key)
            .map(|(_, stamp)| *stamp)
            .unwrap_or_default()
            .after(time, origin)
    }

    /// The keys in `range` ever written, in order, each with its rank and its value, or none
    /// for one removed.
    pub fn written_range<R: std::ops::RangeBounds<K>>(
        &self,
        range: R,
    ) -> impl Iterator<Item = (&K, u64, Option<&V>)> {
        self.ranks
            .range(range)
            .map(|(key, (rank, _))| (key, *rank, self.entries.get(key)))
    }
}

impl<K, V> Deref for RankedMap<K, V> {
    type Target = BTreeMap<K, V>;

    fn deref(&self) -> &BTreeMap<K, V> {
        &self.entries
    }
}

impl<K: Serialize + DeserializeOwned + Ord + Clone, V: Serialize + DeserializeOwned> Field
    for RankedMap<K, V>
{
    fn blank() -> Value {
        Value::Ranked(BTreeMap::new())
    }

    fn shape() -> Shape {
        Shape::Ranked
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        let Value::Ranked(held) = value else {
            return Self::default();
        };
        let mut map = Self::default();
        for (key, ranked) in held {
            let Some(key) = decode::<K>(key) else {
                continue;
            };
            map.ranks.insert(key.clone(), ranked.order());
            if let Some(value) = ranked.value.as_deref().and_then(decode) {
                map.entries.insert(key, value);
            }
        }
        map
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Ranked(
            self.ranks
                .iter()
                .map(|(key, (rank, stamp))| {
                    (
                        encode(key),
                        Ranked {
                            rank: *rank,
                            stamp: *stamp,
                            value: self.entries.get(key).map(encode),
                        },
                    )
                })
                .collect(),
        )
    }
}

impl<M, K: Serialize, V: Serialize> FieldRef<M, RankedMap<K, V>> {
    /// Writes `value` (or a removal) at `key` and `rank`, which takes effect only if it ranks
    /// higher than what is held: with a higher rank, or the same rank and a later `stamp`.
    pub fn rank(
        self,
        object: ObjectId,
        key: &K,
        rank: u64,
        value: Option<&V>,
        stamp: Stamp,
    ) -> Change {
        Change::Rank {
            object,
            field: self.index(),
            key: encode(key),
            ranked: Ranked {
                rank,
                stamp,
                value: value.map(encode),
            },
        }
    }
}
