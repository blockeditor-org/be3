use std::{collections::BTreeMap, fmt, marker::PhantomData};

use be_block_model::{BlockRef, Stored, from_cbor, to_cbor};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub const MAX_NAME_BYTES: usize = 128;

pub const NAME: MetaKey<String> =
    MetaKey::defined(0x6be3_6d65_7461_4e00_8a1e_6e61_6d65_0001, "name");

pub const ICON: MetaKey<String> =
    MetaKey::defined(0x6be3_6d65_7461_4900_8a1e_6963_6f6e_0002, "icon_name");

pub const MATERIAL_ICON: MetaKey<char> =
    MetaKey::defined(0x6be3_6d65_7461_4900_8a1e_6963_6f6e_0001, "icon");

const MATERIAL_ICON_NAMES: [(char, &str); 16] = [
    ('\u{e873}', "file-text"),
    ('\u{e2c7}', "folder"),
    ('\u{f09a}', "star"),
    ('\u{e87e}', "heart"),
    ('\u{e9b2}', "house"),
    ('\u{e943}', "briefcase"),
    ('\u{e80c}', "graduation-cap"),
    ('\u{e86e}', "book"),
    ('\u{e86f}', "code"),
    ('\u{e90f}', "lightbulb"),
    ('\u{f0c6}', "flag"),
    ('\u{ebcc}', "calendar-days"),
    ('\u{e6b1}', "list-checks"),
    ('\u{e3f4}', "image"),
    ('\u{e405}', "eighth-note"),
    ('\u{e80b}', "globe"),
];

pub const ICON_COLOR: MetaKey<[u8; 3]> =
    MetaKey::defined(0x6be3_6d65_7461_4900_8a1e_636f_6c72_0001, "icon_color");

pub const INTRINSIC_NAME: MetaKey<String> =
    MetaKey::derived(0x6be3_6d65_7461_4e00_8a1e_6e61_6d65_0002, "intrinsic_name");

pub const THUMBHASH: MetaKey<Thumbhash> =
    MetaKey::derived(0x6be3_6d65_7461_5400_8a1e_7468_756d_0001, "thumbhash");

pub const ROW_PREFIX: MetaKey<String> =
    MetaKey::derived(0x6be3_6d65_7461_5000_8a1e_7072_6566_0001, "row_prefix");

pub const SCHEMA: MetaKey<String> =
    MetaKey::derived(0x6be3_6d65_7461_5300_8a1e_7363_6865_0001, "schema");

pub const ARTIFACT: MetaKey<Artifact> =
    MetaKey::defined(0x6be3_6d65_7461_4100_8a1e_6172_7469_0001, "artifact");

pub const LOCAL_ID: MetaKey<Uuid> =
    MetaKey::defined(0x6be3_6d65_7461_4c00_8a1e_6c6f_6361_0001, "local_id");

pub const MY_FILES: MetaKey<bool> =
    MetaKey::defined(0x6be3_6d65_7461_4d00_8a1e_6d79_6669_0001, "my_files");

pub const EXECUTABLE: MetaKey<bool> =
    MetaKey::defined(0x6be3_6d65_7461_5800_8a1e_6578_6563_0001, "executable");

pub const DELETED_FROM: MetaKey<DeletedFrom> =
    MetaKey::defined(0x6be3_6d65_7461_4400_8a1e_6465_6c66_0001, "deleted_from");

pub const SETTINGS: MetaKey<BlockRef> =
    MetaKey::defined(0x6be3_6d65_7461_5300_8a1e_7365_7474_0001, "settings");

/// The block's search index (plans/search.md): a block of `be_block_graph::SEARCH_INDEX_TYPE`
/// under it holding the text of its whole tree. Held by its block like `SETTINGS`, so never
/// listed among its children.
pub const SEARCH_INDEX: MetaKey<BlockRef> =
    MetaKey::defined(0x6be3_6d65_7461_4900_8a1e_7365_6172_0001, "search_index");

/// The manifest id of the app that made the block, set by the host when an app creates one
/// and never by the app itself.
pub const CREATED_BY: MetaKey<String> =
    MetaKey::defined(0x6be3_6d65_7461_4300_8a1e_6372_6279_0001, "created_by");

pub const VIEW_EDITOR: MetaKey<Uuid> =
    MetaKey::defined(0x6be3_6d65_7461_5600_8a1e_7669_6577_0001, "view_editor");

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
    #[serde(with = "be_block_model::bytes")]
    pub hash: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Artifact {
    pub source: BlockRef,
    pub source_type: Uuid,
    #[serde(with = "be_block_model::bytes")]
    pub settings: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DeletedFrom {
    Root,
    Block(Uuid),
}

impl BlockMetadata {
    pub fn named(name: impl Into<String>) -> Self {
        let mut metadata = Self::default();
        metadata.insert(&NAME, &name.into());
        metadata
    }

    pub fn view(editor: Uuid, name: Option<String>) -> Self {
        let mut metadata = Self::default();
        metadata.put(&NAME, name.as_ref());
        metadata.insert(&VIEW_EDITOR, &editor);
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

    pub fn icon(&self) -> Option<String> {
        self.get(&ICON).or_else(|| {
            let material = self.get(&MATERIAL_ICON)?;
            MATERIAL_ICON_NAMES
                .iter()
                .find(|(codepoint, _)| *codepoint == material)
                .map(|(_, name)| (*name).to_owned())
        })
    }

    pub fn references(&self) -> Vec<Uuid> {
        let mut found = Vec::new();
        for value in self.defined.values().chain(self.derived.values()) {
            if let Some(value) = from_cbor::<Stored>(value) {
                be_block_model::references::walk(&value, &mut found);
            }
        }
        let mut seen = std::collections::HashSet::new();
        found.retain(|id| seen.insert(*id));
        found
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
        let bytes = be_block_model::bytes::deserialize(deserializer)?;
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

/// The longest name a dialog quotes, in characters, before it ends it with an ellipsis.
const MAX_QUOTED_NAME: usize = 60;

/// A block's name as a dialog quotes it inside its own curly quotes: without control or format
/// characters (newlines, bidi overrides, zero-width marks), its whitespace collapsed, curly
/// quotes made straight so a name cannot close the dialog's quote and fake what follows, and
/// cut at `MAX_QUOTED_NAME` characters.
pub fn quoted_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter_map(|c| match c {
            c if c.is_whitespace() => Some(' '),
            c if c.is_control() || is_format(c) => None,
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' | '\u{00ab}' | '\u{00bb}'
            | '\u{2033}' | '\u{2036}' | '\u{275d}' | '\u{275e}' | '\u{301d}'..='\u{301f}'
            | '\u{ff02}' => Some('"'),
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' | '\u{2032}' | '\u{2035}'
            | '\u{2039}' | '\u{203a}' | '\u{275b}' | '\u{275c}' | '\u{ff07}' => Some('\''),
            c => Some(c),
        })
        .collect();
    let cleaned = cap_combining(&cleaned);
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.chars().count() > MAX_QUOTED_NAME {
        true => {
            let kept: String = collapsed.chars().take(MAX_QUOTED_NAME - 1).collect();
            format!("{}\u{2026}", kept.trim_end())
        }
        false => collapsed,
    }
}

/// The most combining marks a quoted name keeps on one character: enough for any script, too
/// few to stack marks over the dialog's own words.
const MAX_COMBINING: usize = 2;

/// `name` with each run of combining marks cut to `MAX_COMBINING`.
fn cap_combining(name: &str) -> String {
    let mut run = 0;
    name.chars()
        .filter(|c| match is_combining(*c) {
            true => {
                run += 1;
                run <= MAX_COMBINING
            }
            false => {
                run = 0;
                true
            }
        })
        .collect()
}

/// The blocks of combining marks (Combining Diacritical Marks and their Extended, Supplement,
/// for Symbols and Half Marks blocks), which is where stacked marks come from.
fn is_combining(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

/// Unicode's format characters (general category Cf), as of Unicode 15.1.
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}
