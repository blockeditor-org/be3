use be_model::{BlockRef, Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::metadata::MetaKey;
use crate::settings::{Follows, KindVisibility, Layered, union};
use crate::{ChildChange, Root};

pub use be_graph::{EDIT_KINDS, PREFIXES_TYPE, TAGS_TYPE};

pub const fn edit_kind_key(kind: Uuid) -> MetaKey<BlockRef> {
    MetaKey::defined(kind.as_u128(), "edit_kind")
}

pub const PREFIXES: MetaKey<BlockRef> = edit_kind_key(PREFIXES_TYPE);

pub const TAG_DEFINITIONS: MetaKey<BlockRef> = edit_kind_key(TAGS_TYPE);

pub const TAGGED: MetaKey<Vec<BlockRef>> =
    MetaKey::defined(0x6be3_6d65_7461_5400_8a1e_7461_6773_0001, "tagged");

pub const TAG_COLOR: MetaKey<TagColor> =
    MetaKey::derived(0x6be3_6d65_7461_5400_8a1e_7461_6763_0001, "tag_color");

pub const PREFERRED_EDITOR: MetaKey<Uuid> = MetaKey::defined(
    0x6be3_6d65_7461_4500_8a1e_6564_6974_0001,
    "preferred_editor",
);

pub fn normal_prefix(prefix: &str) -> String {
    prefix.trim().to_ascii_uppercase()
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct PrefixRegistry {
    pub databases: Map<Uuid, String>,
}

impl PrefixRegistry {
    pub fn databases_with(&self, prefix: &str) -> Vec<Uuid> {
        let wanted = normal_prefix(prefix);
        self.databases
            .iter()
            .filter(|(_, held)| !wanted.is_empty() && normal_prefix(held) == wanted)
            .map(|(database, _)| *database)
            .collect()
    }

    pub fn prefix_of(&self, database: Uuid) -> Option<&str> {
        self.databases.get(&database).map(String::as_str)
    }

    pub fn prefixes(&self) -> Vec<(String, Vec<Uuid>)> {
        let mut grouped: std::collections::BTreeMap<String, Vec<Uuid>> =
            std::collections::BTreeMap::new();
        for (database, prefix) in self.databases.iter() {
            let prefix = normal_prefix(prefix);
            if !prefix.is_empty() {
                grouped.entry(prefix).or_default().push(*database);
            }
        }
        grouped.into_iter().collect()
    }

    pub fn register(database: Uuid, prefix: Option<&str>) -> Edit {
        let prefix = prefix
            .map(normal_prefix)
            .filter(|prefix| !prefix.is_empty());
        Self::DATABASES
            .put(ObjectId::ROOT, &database, prefix.as_ref())
            .into()
    }
}

impl Root for PrefixRegistry {
    const CONTENT_TYPE: Uuid = PREFIXES_TYPE;
}

impl Layered for PrefixRegistry {
    const NAME: &'static str = "Database prefixes";
    const FOLLOWS: Follows = Follows::Block;
    const VISIBILITY: KindVisibility = KindVisibility::Inside;

    fn over(&self, lower: &Self) -> Self {
        Self {
            databases: union(&self.databases, &lower.databases),
        }
    }
}

pub type PrefixRegistryContent = Document<PrefixRegistry>;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum TagColor {
    #[default]
    Gray,
    Blue,
    Teal,
    Green,
    Yellow,
    Orange,
    Red,
    Pink,
    Purple,
}

impl TagColor {
    pub const ALL: [Self; 9] = [
        Self::Gray,
        Self::Blue,
        Self::Teal,
        Self::Green,
        Self::Yellow,
        Self::Orange,
        Self::Red,
        Self::Pink,
        Self::Purple,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Gray => "Gray",
            Self::Blue => "Blue",
            Self::Teal => "Teal",
            Self::Green => "Green",
            Self::Yellow => "Yellow",
            Self::Orange => "Orange",
            Self::Red => "Red",
            Self::Pink => "Pink",
            Self::Purple => "Purple",
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Tag {
    pub name: String,
    pub color: TagColor,
}

impl Tag {
    pub fn rename(name: &str) -> Edit {
        Self::NAME.set(ObjectId::ROOT, &name.to_owned()).into()
    }

    pub fn recolor(color: TagColor) -> Edit {
        Self::COLOR.set(ObjectId::ROOT, &color).into()
    }
}

impl Root for Tag {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7461_672d_626c_6f63_6b2d_6265_3364_3031);

    fn name(&self) -> Option<String> {
        Some(self.name.clone()).filter(|name| !name.trim().is_empty())
    }

    fn describe(&self, metadata: &mut crate::BlockMetadata) {
        metadata.insert(&TAG_COLOR, &self.color);
    }
}

pub type TagContent = Document<Tag>;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Tags {
    pub tags: Map<Uuid, ()>,
}

impl Tags {
    pub fn tags(&self) -> Vec<Uuid> {
        self.tags.keys().copied().collect()
    }

    pub fn add(tag: Uuid) -> Edit {
        Self::TAGS.put(ObjectId::ROOT, &tag, Some(&())).into()
    }

    pub fn remove(tag: Uuid) -> Edit {
        Self::TAGS.put(ObjectId::ROOT, &tag, None).into()
    }
}

impl Root for Tags {
    const CONTENT_TYPE: Uuid = TAGS_TYPE;

    fn references(&self) -> Vec<Uuid> {
        self.tags()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(tag) => Some(Self::add(tag)),
            ChildChange::Delete(tag) => Some(Self::remove(tag)),
            ChildChange::Replace { old, new } => Some(
                [
                    Self::TAGS.put(ObjectId::ROOT, &old, None),
                    Self::TAGS.put(ObjectId::ROOT, &new, Some(&())),
                ]
                .into_iter()
                .collect(),
            ),
        }
    }
}

impl Layered for Tags {
    const NAME: &'static str = "Tags";
    const FOLLOWS: Follows = Follows::Block;
    const VISIBILITY: KindVisibility = KindVisibility::Inside;

    fn over(&self, lower: &Self) -> Self {
        Self {
            tags: union(&self.tags, &lower.tags),
        }
    }
}

pub type TagsContent = Document<Tags>;

pub fn tagged(metadata: &crate::BlockMetadata) -> Vec<Uuid> {
    metadata
        .get(&TAGGED)
        .unwrap_or_default()
        .into_iter()
        .map(|tag| tag.0)
        .collect()
}

pub fn retagged(metadata: &crate::BlockMetadata, tag: Uuid, on: bool) -> Vec<BlockRef> {
    let mut tags = tagged(metadata);
    tags.retain(|held| *held != tag);
    if on {
        tags.push(tag);
    }
    tags.into_iter().map(BlockRef).collect()
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Home {
    pub landing: Option<BlockRef>,
    pub pinned: Map<Uuid, ()>,
    pub templates: Map<Uuid, ()>,
}

impl Home {
    pub fn pinned(&self) -> Vec<Uuid> {
        self.pinned.keys().copied().collect()
    }

    pub fn templates(&self) -> Vec<Uuid> {
        self.templates.keys().copied().collect()
    }

    pub fn set_landing(block: Option<Uuid>) -> Edit {
        Self::LANDING
            .set(ObjectId::ROOT, &block.map(BlockRef))
            .into()
    }

    pub fn pin(block: Uuid, pinned: bool) -> Edit {
        Self::PINNED
            .put(ObjectId::ROOT, &block, pinned.then_some(&()))
            .into()
    }

    pub fn keep_template(block: Uuid, kept: bool) -> Edit {
        Self::TEMPLATES
            .put(ObjectId::ROOT, &block, kept.then_some(&()))
            .into()
    }
}

impl Root for Home {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x686f_6d65_2d6b_696e_642d_6265_3364_3031);

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self
            .landing
            .iter()
            .map(|landing| landing.0)
            .chain(self.pinned())
            .chain(self.templates())
            .collect();
        references.sort_unstable();
        references.dedup();
        references
    }
}

impl Layered for Home {
    const NAME: &'static str = "Home";
    const FOLLOWS: Follows = Follows::Block;

    fn over(&self, lower: &Self) -> Self {
        Self {
            landing: self.landing.or(lower.landing),
            pinned: match self.pinned.is_empty() {
                true => lower.pinned.clone(),
                false => self.pinned.clone(),
            },
            templates: union(&self.templates, &lower.templates),
        }
    }
}

pub type HomeContent = Document<Home>;
