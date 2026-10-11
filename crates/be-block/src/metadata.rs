use std::{collections::BTreeMap, fmt, marker::PhantomData};

use be_model::{Stored, from_cbor, to_cbor};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub const MAX_NAME_BYTES: usize = 128;

pub const NAME: MetaKey<String> =
    MetaKey::defined(0x6be3_6d65_7461_4e00_8a1e_6e61_6d65_0001, "name");

pub const INTRINSIC_NAME: MetaKey<String> =
    MetaKey::derived(0x6be3_6d65_7461_4e00_8a1e_6e61_6d65_0002, "intrinsic_name");

pub const THUMBHASH: MetaKey<Thumbhash> =
    MetaKey::derived(0x6be3_6d65_7461_5400_8a1e_7468_756d_0001, "thumbhash");

pub const ARTIFACT: MetaKey<ArtifactSource> =
    MetaKey::defined(0x6be3_6d65_7461_4100_8a1e_6172_7469_0001, "artifact");

pub const LOCAL_ID: MetaKey<Uuid> =
    MetaKey::defined(0x6be3_6d65_7461_4c00_8a1e_6c6f_6361_0001, "local_id");

const DEFINED: &str = "defined";
const DERIVED: &str = "derived";
const UUID_TAG: u64 = 37;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    Defined,
    Derived,
}

pub struct MetaKey<T> {
    pub id: Uuid,
    pub name: &'static str,
    pub origin: Origin,
    value: PhantomData<fn() -> T>,
}

impl<T> MetaKey<T> {
    pub const fn defined(id: u128, name: &'static str) -> Self {
        Self::new(id, name, Origin::Defined)
    }

    pub const fn derived(id: u128, name: &'static str) -> Self {
        Self::new(id, name, Origin::Derived)
    }

    const fn new(id: u128, name: &'static str, origin: Origin) -> Self {
        Self {
            id: Uuid::from_u128(id),
            name,
            origin,
            value: PhantomData,
        }
    }
}

impl<T: Serialize> MetaKey<T> {
    pub fn definition(&self, value: Option<&T>) -> Definition {
        (self.id, value.map(to_cbor))
    }
}

impl<T> Clone for MetaKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for MetaKey<T> {}

impl<T> fmt::Debug for MetaKey<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MetaKey({} {})", self.name, self.id)
    }
}

pub type Entries = BTreeMap<Uuid, Vec<u8>>;

pub type Definition = (Uuid, Option<Vec<u8>>);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockMetadata {
    pub defined: Entries,
    pub derived: Entries,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Thumbhash {
    #[serde(with = "be_model::bytes")]
    pub hash: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactSource {
    pub source_type: Uuid,
    #[serde(with = "be_model::bytes")]
    pub data: Vec<u8>,
}

impl BlockMetadata {
    pub fn named(name: impl Into<String>) -> Self {
        let mut metadata = Self::default();
        metadata.insert(&NAME, &name.into());
        metadata
    }

    pub fn get<T: DeserializeOwned>(&self, key: &MetaKey<T>) -> Option<T> {
        from_cbor(self.entries(key.origin).get(&key.id)?)
    }

    pub fn contains<T>(&self, key: &MetaKey<T>) -> bool {
        self.entries(key.origin).contains_key(&key.id)
    }

    pub fn insert<T: Serialize>(&mut self, key: &MetaKey<T>, value: &T) {
        self.entries_mut(key.origin).insert(key.id, to_cbor(value));
    }

    pub fn remove<T>(&mut self, key: &MetaKey<T>) {
        self.entries_mut(key.origin).remove(&key.id);
    }

    pub fn put<T: Serialize>(&mut self, key: &MetaKey<T>, value: Option<&T>) {
        match value {
            Some(value) => self.insert(key, value),
            None => self.remove(key),
        }
    }

    pub fn define(&mut self, definitions: &[Definition]) {
        for (key, value) in definitions {
            match value {
                Some(value) => self.defined.insert(*key, value.clone()),
                None => self.defined.remove(key),
            };
        }
    }

    pub fn name(&self) -> Option<String> {
        self.get(&NAME)
            .or_else(|| self.get(&INTRINSIC_NAME))
            .filter(|name| !name.is_empty())
    }

    pub fn named_by_hand(&self) -> bool {
        self.contains(&NAME)
    }

    pub fn with_defined(&self, defined: Entries) -> Self {
        Self {
            defined,
            derived: self.derived.clone(),
        }
    }

    pub fn with_derived(&self, derived: Entries) -> Self {
        Self {
            defined: self.defined.clone(),
            derived,
        }
    }

    fn entries(&self, origin: Origin) -> &Entries {
        match origin {
            Origin::Defined => &self.defined,
            Origin::Derived => &self.derived,
        }
    }

    fn entries_mut(&mut self, origin: Origin) -> &mut Entries {
        match origin {
            Origin::Defined => &mut self.defined,
            Origin::Derived => &mut self.derived,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        if self.defined.is_empty() && self.derived.is_empty() {
            return Vec::new();
        }
        to_cbor(&Stored::Map(vec![
            (
                Stored::Text(DEFINED.to_owned()),
                stored_entries(&self.defined),
            ),
            (
                Stored::Text(DERIVED.to_owned()),
                stored_entries(&self.derived),
            ),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, crate::ContentError> {
        let malformed = || crate::ContentError::Malformed("block metadata");
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let Some(Stored::Map(sections)) = from_cbor::<Stored>(bytes) else {
            return Err(malformed());
        };
        let mut metadata = Self::default();
        for (section, entries) in sections {
            let target = match section.as_text() {
                Some(DEFINED) => &mut metadata.defined,
                Some(DERIVED) => &mut metadata.derived,
                _ => continue,
            };
            *target = read_entries(entries).ok_or_else(malformed)?;
        }
        Ok(metadata)
    }

    pub fn merge_defined(base: &Self, ours: &Self, theirs: &Self) -> (Entries, Vec<Uuid>) {
        let mut merged = Entries::new();
        let mut conflicts = Vec::new();
        let keys: std::collections::BTreeSet<&Uuid> = base
            .defined
            .keys()
            .chain(ours.defined.keys())
            .chain(theirs.defined.keys())
            .collect();
        for key in keys {
            let before = base.defined.get(key);
            let mine = ours.defined.get(key);
            let other = theirs.defined.get(key);
            let chosen = if mine == other || other == before {
                mine
            } else if mine == before {
                other
            } else {
                conflicts.push(*key);
                mine
            };
            if let Some(value) = chosen {
                merged.insert(*key, value.clone());
            }
        }
        (merged, conflicts)
    }
}

impl Serialize for BlockMetadata {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.encode())
    }
}

impl<'de> Deserialize<'de> for BlockMetadata {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = be_model::bytes::deserialize(deserializer)?;
        Self::decode(&bytes).map_err(serde::de::Error::custom)
    }
}

fn stored_entries(entries: &Entries) -> Stored {
    Stored::Map(
        entries
            .iter()
            .map(|(key, value)| {
                (
                    Stored::Tag(UUID_TAG, Box::new(Stored::Bytes(key.as_bytes().to_vec()))),
                    from_cbor::<Stored>(value).unwrap_or(Stored::Null),
                )
            })
            .collect(),
    )
}

fn read_entries(entries: Stored) -> Option<Entries> {
    let Stored::Map(entries) = entries else {
        return None;
    };
    entries
        .into_iter()
        .map(|(key, value)| {
            let Stored::Tag(UUID_TAG, key) = key else {
                return None;
            };
            let key = Uuid::from_slice(key.as_bytes()?).ok()?;
            Some((key, to_cbor(&value)))
        })
        .collect()
}
