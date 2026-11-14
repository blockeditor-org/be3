//! `Sections`, a block type for tests of types that load and merge by parts: ten sections of
//! bytes, each saved as a part of its own after a header of their lengths, which is the first
//! part and all a replica loads. Its plain file is one line per section (`*.sections`), and
//! its searchable text one document per section, keyed by its index.

use std::collections::{HashMap, HashSet};

use be_block_commit::MergeResult;
use be_block_content::{
    BlockContent, ContentError, LiveEdit, Loads, Merge, MergeParts, Part, PartReader, PartRef,
    SearchDocument, Stored,
};
use be_sdk_type_api::PlainFile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_sdk_type_guest::FileForm;

pub const SECTIONS: usize = 10;

const HEADER: usize = SECTIONS * 4;

#[derive(Clone, Debug)]
enum Section {
    Held {
        bytes: Vec<u8>,
        /// The part it was loaded from, which a save reuses.
        saved: Option<PartRef>,
    },
    Unloaded(PartRef),
}

impl Section {
    fn saved(&self) -> Option<PartRef> {
        match self {
            Self::Held { saved, .. } => *saved,
            Self::Unloaded(saved) => Some(*saved),
        }
    }

    fn length(&self) -> u64 {
        match self {
            Self::Held { bytes, .. } => bytes.len() as u64,
            Self::Unloaded(saved) => saved.length(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sections {
    sections: Vec<Section>,
}

impl Default for Sections {
    fn default() -> Self {
        Self::of(&[""; SECTIONS])
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SetSection {
    pub index: usize,
    pub bytes: Vec<u8>,
}

impl Sections {
    pub fn of(texts: &[&str; SECTIONS]) -> Self {
        Self {
            sections: texts
                .iter()
                .map(|text| Section::Held {
                    bytes: text.as_bytes().to_vec(),
                    saved: None,
                })
                .collect(),
        }
    }

    pub fn set(&mut self, index: usize, text: &str) {
        self.apply(&SetSection {
            index,
            bytes: text.as_bytes().to_vec(),
        });
    }

    /// A section's bytes; none while it is not loaded.
    pub fn section(&self, index: usize) -> Option<&[u8]> {
        match self.sections.get(index)? {
            Section::Held { bytes, .. } => Some(bytes),
            Section::Unloaded(_) => None,
        }
    }

    /// The part a section was loaded from.
    pub fn saved(&self, index: usize) -> Option<PartRef> {
        self.sections.get(index)?.saved()
    }

    fn header(&self) -> Vec<u8> {
        self.sections
            .iter()
            .flat_map(|section| (section.length() as u32).to_le_bytes())
            .collect()
    }
}

impl BlockContent for Sections {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x5ec7_1095_0000_4000_8000_7e57_0000_0010);

    const LOADS: Loads = Loads::FirstPart;

    /// Leaves out the bytes of sections not loaded: only `parts` writes such content whole.
    fn encode(&self) -> Vec<u8> {
        let mut bytes = self.header();
        for section in &self.sections {
            if let Section::Held { bytes: held, .. } = section {
                bytes.extend_from_slice(held);
            }
        }
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let header = bytes.get(..HEADER).ok_or(ContentError::Truncated)?;
        let mut at = HEADER;
        let mut sections = Vec::with_capacity(SECTIONS);
        for length in header.chunks_exact(4) {
            let length = u32::from_le_bytes(length.try_into().unwrap_or_default()) as usize;
            let held = bytes.get(at..at + length).ok_or(ContentError::Truncated)?;
            at += length;
            sections.push(Section::Held {
                bytes: held.to_vec(),
                saved: None,
            });
        }
        Ok(Self { sections })
    }

    fn parts(&self) -> Part<'_> {
        let mut parts = vec![Part::from(self.header())];
        parts.extend(self.sections.iter().map(|section| match section {
            Section::Held {
                saved: Some(saved), ..
            }
            | Section::Unloaded(saved) => Part::Reused(*saved),
            Section::Held { bytes, saved: None } => Part::from(bytes.as_slice()),
        }));
        Part::node(parts)
    }

    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        let Some(root) = stored
            .root
            .as_ref()
            .filter(|root| root.children.len() == SECTIONS + 1)
        else {
            return Self::decode(&stored.bytes);
        };
        Ok(Self {
            sections: root.children[1..]
                .iter()
                .map(|part| match stored.holds(part) {
                    true => Section::Held {
                        bytes: stored.bytes_of(part).to_vec(),
                        saved: Some(part.reference),
                    },
                    false => Section::Unloaded(part.reference),
                })
                .collect(),
        })
    }

    /// Reads a section only when it is asked for, unloaded and not indexed.
    fn search_text(
        &self,
        keys: Option<&HashSet<String>>,
        indexed: &HashSet<PartRef>,
        parts: PartReader,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<SearchDocument>, String>>>> {
        let asked: Vec<(String, Option<PartRef>, Option<Section>)> = self
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| (index.to_string(), section))
            .filter(|(key, _)| keys.is_none_or(|keys| keys.contains(key)))
            .map(|(key, section)| {
                let source = section.saved();
                let unindexed = !source.is_some_and(|source| indexed.contains(&source));
                (key, source, unindexed.then(|| section.clone()))
            })
            .collect();
        Box::pin(async move {
            let mut documents = Vec::with_capacity(asked.len());
            for (key, source, section) in asked {
                let body = match section {
                    None => None,
                    Some(Section::Held { bytes, .. }) => Some(bytes),
                    Some(Section::Unloaded(part)) => Some(parts.read_all(part).await?),
                };
                documents.push(SearchDocument {
                    key,
                    source,
                    name: None,
                    body: body.map(|body| String::from_utf8_lossy(&body).into_owned()),
                });
            }
            Ok(documents)
        })
    }
}

impl LiveEdit for Sections {
    type Op = SetSection;

    fn apply(&mut self, operation: &SetSection) {
        if let Some(section) = self.sections.get_mut(operation.index) {
            *section = Section::Held {
                bytes: operation.bytes.clone(),
                saved: None,
            };
        }
    }
}

/// One section's three sides as bytes: a side that equals the base gives way to the other,
/// and two different changes keep ours as a conflict.
fn merge_bytes(base: &[u8], ours: &[u8], theirs: &[u8]) -> (Vec<u8>, bool) {
    if ours == base || ours == theirs {
        return (theirs.to_vec(), false);
    }
    if theirs == base {
        return (ours.to_vec(), false);
    }
    (ours.to_vec(), true)
}

fn merged(sections: Vec<Section>, conflicts: usize) -> MergeResult<Sections> {
    let value = Sections { sections };
    match conflicts {
        0 => MergeResult::Clean(value),
        conflicts => MergeResult::Conflicted { value, conflicts },
    }
}

/// What a section's refs settle without its bytes: a side saved as the base's part is
/// unchanged, so the other side wins (equal refs mean equal bytes, though not the other way
/// round: a rotation's reseal gives the same bytes a new ref).
fn by_refs(base: &Section, ours: &Section, theirs: &Section) -> Option<Section> {
    let base = base.saved()?;
    if ours.saved() == Some(base) {
        return Some(theirs.clone());
    }
    if theirs.saved() == Some(base) {
        return Some(ours.clone());
    }
    None
}

impl Merge for Sections {
    /// Over what each side holds: a section unloaded on a side that its refs do not settle
    /// keeps ours as a conflict, where `merge_parts` reads it. A replica merges with this in
    /// its session (guides/type_modules.md, "Merging by parts").
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let mut conflicts = 0;
        let mut sections = Vec::with_capacity(SECTIONS);
        for index in 0..SECTIONS {
            let [base, ours, theirs] = [base, ours, theirs].map(|side| &side.sections[index]);
            if let Some(settled) = by_refs(base, ours, theirs) {
                sections.push(settled);
                continue;
            }
            sections.push(match (base, ours, theirs) {
                (
                    Section::Held { bytes: base, .. },
                    Section::Held { bytes: ours, .. },
                    Section::Held { bytes: theirs, .. },
                ) => {
                    let (bytes, conflicted) = merge_bytes(base, ours, theirs);
                    conflicts += usize::from(conflicted);
                    Section::Held { bytes, saved: None }
                }
                _ => {
                    conflicts += 1;
                    ours.clone()
                }
            });
        }
        merged(sections, conflicts)
    }
}

impl MergeParts for Sections {
    /// A section its refs settle is kept as the winning side's part, unread. Any other is
    /// merged from its bytes, reading each side's part once (sides that share a part share
    /// the read), and written anew.
    fn merge_parts(
        base: Self,
        ours: Self,
        theirs: Self,
        parts: PartReader,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<MergeResult<Self>, String>>>> {
        Box::pin(async move {
            let mut read: HashMap<PartRef, Vec<u8>> = HashMap::new();
            let mut conflicts = 0;
            let mut sections = Vec::with_capacity(SECTIONS);
            for index in 0..SECTIONS {
                let sides = [&base, &ours, &theirs].map(|side| side.sections[index].clone());
                if let Some(settled) = by_refs(&sides[0], &sides[1], &sides[2]) {
                    sections.push(settled);
                    continue;
                }
                let mut bytes = Vec::with_capacity(3);
                for side in sides {
                    bytes.push(match side {
                        Section::Held { bytes, .. } => bytes,
                        Section::Unloaded(part) => {
                            if !read.contains_key(&part) {
                                read.insert(part, parts.read_all(part).await?);
                            }
                            read[&part].clone()
                        }
                    });
                }
                let (bytes, conflicted) = merge_bytes(&bytes[0], &bytes[1], &bytes[2]);
                conflicts += usize::from(conflicted);
                sections.push(Section::Held { bytes, saved: None });
            }
            Ok(merged(sections, conflicts))
        })
    }
}

impl FileForm for Sections {
    fn to_file(&self) -> Option<PlainFile> {
        let mut bytes = Vec::new();
        for index in 0..SECTIONS {
            bytes.extend_from_slice(self.section(index)?);
            bytes.push(b'\n');
        }
        Some(PlainFile {
            bytes,
            name: None,
            extension: "sections".to_owned(),
            media_type: "text/plain".to_owned(),
        })
    }

    fn from_file(_: Option<&Self>, _: &str, bytes: Vec<u8>, _: u64) -> Option<Self> {
        let text = String::from_utf8(bytes).ok()?;
        let mut lines = text.lines();
        let texts: [&str; SECTIONS] = std::array::from_fn(|_| lines.next().unwrap_or_default());
        Some(Self::of(&texts))
    }

    fn claims(name: &str, _: &[u8]) -> bool {
        name.ends_with(".sections")
    }
}

be_sdk_type_guest::be_block_types![Sections { file, merge_parts }];
