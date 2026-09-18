use be_commit::{MergeResult, merge_lines, render_conflicts};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, LiveEdit, Merge, Streamed, decode_streamed, encode_streamed,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum TextLanguage {
    #[default]
    Markdown,
    PlainText,
    Rust,
    Zig,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextHeader {
    pub language: TextLanguage,
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
    SetLanguage(TextLanguage),
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
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7465_7874_2d62_6c6f_636b_2d74_7970_6502);

    fn encode(&self) -> Vec<u8> {
        encode_streamed(&self.header, &self.bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let (header, bytes) = decode_streamed(bytes)?;
        Ok(Self { header, bytes })
    }

    fn name(&self) -> Option<String> {
        let line = self.bytes.split(|byte| *byte == b'\n').next()?;
        let name = String::from_utf8_lossy(line).trim().to_owned();
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

impl LiveEdit for TextContent {
    type Op = TextOp;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            TextOp::Insert { at, bytes } => {
                let at = (*at as usize).min(self.bytes.len());
                self.bytes.splice(at..at, bytes.iter().copied());
            }
            TextOp::Delete { at, length } => {
                let at = (*at as usize).min(self.bytes.len());
                let end = at.saturating_add(*length as usize).min(self.bytes.len());
                self.bytes.drain(at..end);
            }
            TextOp::SetLanguage(language) => self.header.language = *language,
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
        (operation, TextOp::SetLanguage(_)) => Some(operation),
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
        (
            TextOp::Insert { at, bytes },
            TextOp::Delete {
                at: their_at,
                length,
            },
        ) => {
            let at = if their_at + length <= at {
                at - length
            } else if *their_at < at {
                *their_at
            } else {
                at
            };
            Some(TextOp::Insert { at, bytes })
        }
        (
            TextOp::Delete { at, length },
            TextOp::Insert {
                at: their_at,
                bytes,
            },
        ) => {
            let inserted = bytes.len() as u64;
            if *their_at <= at {
                Some(TextOp::Delete {
                    at: at + inserted,
                    length,
                })
            } else if *their_at < at + length {
                Some(TextOp::Delete {
                    at,
                    length: length + inserted,
                })
            } else {
                Some(TextOp::Delete { at, length })
            }
        }
        (
            TextOp::Delete { at, length },
            TextOp::Delete {
                at: their_at,
                length: their_length,
            },
        ) => {
            let (their_at, their_length) = (*their_at, *their_length);
            if their_at + their_length <= at {
                return Some(TextOp::Delete {
                    at: at - their_length,
                    length,
                });
            }
            if their_at >= at + length {
                return Some(TextOp::Delete { at, length });
            }
            let overlap = (at + length).min(their_at + their_length) - at.max(their_at);
            let length = length.checked_sub(overlap)?;
            if length == 0 {
                return None;
            }
            Some(TextOp::Delete {
                at: at.min(their_at),
                length,
            })
        }
    }
}

impl Merge for TextContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let language = if ours.header.language == base.header.language {
            theirs.header.language
        } else {
            ours.header.language
        };
        let outcome = merge_lines(&base.bytes, &ours.bytes, &theirs.bytes);
        let header = TextHeader { language };
        if outcome.is_clean() {
            return MergeResult::Clean(Self {
                header,
                bytes: be_commit::merge::join_lines(&outcome.merged),
            });
        }
        MergeResult::Conflicted {
            conflicts: outcome.conflicts.len(),
            value: Self {
                header,
                bytes: render_conflicts(&outcome, "local", "remote"),
            },
        }
    }
}
