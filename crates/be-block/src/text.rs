use be_commit::{MergeResult, merge_lines, render_conflicts};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, LiveEdit, Merge, Streamed, decode_streamed, encode_streamed,
};

const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum TextLanguage {
    #[default]
    Markdown,
    PlainText,
    Rust,
    Zig,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TextIndentation {
    Tabs,
    Spaces { width: u8 },
}

impl Default for TextIndentation {
    fn default() -> Self {
        Self::Spaces { width: 2 }
    }
}

impl TextIndentation {
    fn normalized(self) -> Self {
        match self {
            Self::Tabs => Self::Tabs,
            Self::Spaces { width } => Self::Spaces {
                width: width.max(1),
            },
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextHeader {
    pub language: TextLanguage,
    pub indentation: TextIndentation,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextContent {
    header: TextHeader,
    bytes: Vec<u8>,
}

impl TextContent {
    pub fn new(text: impl Into<Vec<u8>>) -> Self {
        Self {
            header: TextHeader::default(),
            bytes: text.into(),
        }
    }

    pub fn with_language(mut self, language: TextLanguage) -> Self {
        self.header.language = language;
        self
    }

    pub fn language(&self) -> TextLanguage {
        self.header.language
    }

    pub fn indentation(&self) -> TextIndentation {
        self.header.indentation
    }

    pub fn linked_blocks(&self, workspace: Option<Uuid>) -> Vec<Uuid> {
        let mut seen = std::collections::HashSet::new();
        crate::block_url::parse_block_urls(&self.bytes)
            .into_iter()
            .filter(|url| workspace.is_none_or(|workspace| url.workspace_id == workspace))
            .map(|url| url.block)
            .filter(|id| seen.insert(*id))
            .collect()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

impl From<&str> for TextContent {
    fn from(text: &str) -> Self {
        Self::new(text.as_bytes().to_vec())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TextOp {
    Insert { at: u64, bytes: Vec<u8> },
    Delete { at: u64, length: u64 },
    DeleteRanges { ranges: Vec<(u64, u64)> },
    SetLanguage(TextLanguage),
    SetIndentation(TextIndentation),
}

impl TextOp {
    pub fn insert(at: u64, text: &str) -> Self {
        Self::Insert {
            at,
            bytes: text.as_bytes().to_vec(),
        }
    }

    pub fn delete(at: u64, length: u64) -> Self {
        Self::Delete { at, length }
    }
}

impl BlockContent for TextContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_014b);

    fn encode(&self) -> Vec<u8> {
        encode_streamed(&self.header, &self.bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let (header, bytes) = decode_streamed(bytes)?;
        Ok(Self { header, bytes })
    }

    fn references(&self) -> Vec<Uuid> {
        self.linked_blocks(None)
    }

    fn references_in(&self, workspace: Uuid) -> Vec<Uuid> {
        self.linked_blocks(Some(workspace))
    }

    fn name(&self) -> Option<String> {
        let line = self.bytes.split(|byte| *byte == b'\n').next()?;
        let mut name = String::new();
        for character in String::from_utf8_lossy(line).trim().chars() {
            if name.len() + character.len_utf8() > MAX_NAME_BYTES {
                break;
            }
            name.push(character);
        }
        (!name.is_empty()).then_some(name)
    }
}

impl Streamed for TextContent {
    type Header = TextHeader;

    fn header(&self) -> Self::Header {
        self.header.clone()
    }

    fn payload(&self) -> &[u8] {
        &self.bytes
    }

    fn from_parts(header: Self::Header, payload: Vec<u8>) -> Self {
        Self {
            header,
            bytes: payload,
        }
    }
}

impl TextContent {
    fn delete(&mut self, at: u64, length: u64) {
        let at = (at as usize).min(self.bytes.len());
        let end = at.saturating_add(length as usize).min(self.bytes.len());
        self.bytes.drain(at..end);
    }
}

impl LiveEdit for TextContent {
    type Op = TextOp;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            TextOp::Insert { at, bytes } => {
                let at = (*at as usize).min(self.bytes.len());
                self.bytes.splice(at..at, bytes.iter().copied());
            }
            TextOp::Delete { at, length } => self.delete(*at, *length),
            TextOp::DeleteRanges { ranges } => {
                for (at, length) in ranges.iter().rev() {
                    self.delete(*at, *length);
                }
            }
            TextOp::SetLanguage(language) => self.header.language = *language,
            TextOp::SetIndentation(indentation) => {
                self.header.indentation = indentation.normalized();
            }
        }
    }

    fn rebase(operation: Self::Op, onto: &[Self::Op]) -> Option<Self::Op> {
        let mut operation = operation;
        for other in onto {
            operation = transform(operation, other)?;
        }
        Some(operation)
    }
}

fn transform(ours: TextOp, theirs: &TextOp) -> Option<TextOp> {
    match (ours, theirs) {
        (TextOp::SetLanguage(language), _) => Some(TextOp::SetLanguage(language)),
        (TextOp::SetIndentation(indentation), _) => Some(TextOp::SetIndentation(indentation)),
        (operation, TextOp::SetLanguage(_) | TextOp::SetIndentation(_)) => Some(operation),
        (
            TextOp::Insert { at, bytes },
            TextOp::Insert {
                at: their_at,
                bytes: their_bytes,
            },
        ) => {
            let shift = if *their_at <= at {
                their_bytes.len() as u64
            } else {
                0
            };
            Some(TextOp::Insert {
                at: at + shift,
                bytes,
            })
        }
        (TextOp::Insert { at, bytes }, deleted) => {
            let at = spans(deleted)
                .iter()
                .rev()
                .fold(at, |at, (their_at, length)| {
                    if their_at + length <= at {
                        at - length
                    } else if *their_at < at {
                        *their_at
                    } else {
                        at
                    }
                });
            Some(TextOp::Insert { at, bytes })
        }
        (
            deleting,
            TextOp::Insert {
                at: their_at,
                bytes,
            },
        ) => {
            let inserted = bytes.len() as u64;
            let mut ranges = Vec::new();
            for (at, length) in spans(&deleting) {
                if *their_at <= at {
                    ranges.push((at + inserted, length));
                } else if *their_at < at + length {
                    ranges.push((at, their_at - at));
                    ranges.push((their_at + inserted, at + length - their_at));
                } else {
                    ranges.push((at, length));
                }
            }
            deletion(ranges)
        }
        (deleting, deleted) => {
            let theirs = spans(deleted);
            let ranges = spans(&deleting)
                .into_iter()
                .map(|range| {
                    theirs
                        .iter()
                        .rev()
                        .fold(range, |(at, length), (their_at, their_length)| {
                            let end = at + length;
                            let their_end = their_at + their_length;
                            let overlap = end.min(their_end).saturating_sub(at.max(*their_at));
                            let start = if their_end <= at {
                                at - their_length
                            } else {
                                at.min(*their_at)
                            };
                            (start, length - overlap)
                        })
                })
                .collect();
            deletion(ranges)
        }
    }
}

fn spans(operation: &TextOp) -> Vec<(u64, u64)> {
    match operation {
        TextOp::Delete { at, length } => vec![(*at, *length)],
        TextOp::DeleteRanges { ranges } => ranges.clone(),
        _ => Vec::new(),
    }
}

fn deletion(ranges: Vec<(u64, u64)>) -> Option<TextOp> {
    let mut joined: Vec<(u64, u64)> = Vec::new();
    for (at, length) in ranges.into_iter().filter(|(_, length)| *length > 0) {
        match joined.last_mut() {
            Some((last_at, last_length)) if *last_at + *last_length >= at => {
                *last_length = (at + length).max(*last_at + *last_length) - *last_at;
            }
            _ => joined.push((at, length)),
        }
    }
    match joined.as_slice() {
        [] => None,
        [(at, length)] => Some(TextOp::Delete {
            at: *at,
            length: *length,
        }),
        _ => Some(TextOp::DeleteRanges { ranges: joined }),
    }
}

impl Merge for TextContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let mut conflicts = 0;
        let language = pick(
            base.header.language,
            ours.header.language,
            theirs.header.language,
            &mut conflicts,
        );
        let indentation = pick(
            base.header.indentation,
            ours.header.indentation,
            theirs.header.indentation,
            &mut conflicts,
        );
        let outcome = merge_lines(&base.bytes, &ours.bytes, &theirs.bytes);
        let header = TextHeader {
            language,
            indentation,
        };
        let bytes = if outcome.is_clean() {
            be_commit::merge::join_lines(&outcome.merged)
        } else {
            conflicts += outcome.conflicts.len();
            render_conflicts(&outcome, "local", "remote")
        };
        let value = Self { header, bytes };
        if conflicts == 0 {
            MergeResult::Clean(value)
        } else {
            MergeResult::Conflicted { value, conflicts }
        }
    }
}

fn pick<T: PartialEq>(base: T, ours: T, theirs: T, conflicts: &mut usize) -> T {
    if ours == base {
        theirs
    } else {
        if theirs != base && theirs != ours {
            *conflicts += 1;
        }
        ours
    }
}
