//! What the clipboard and drag and drop carry: a clip lists representations of
//! the same content, richest first, each a format id and its data
//! (`ClipItem`). The receiver chooses, by listing the formats it takes; it
//! gets the first the clip holds.
//!
//! Standard formats keep their MIME type and are mapped to each platform's own.
//! be3 formats are the serde types below, encoded with postcard. They are not
//! frozen (guides/frozen_formats.md): a format may change like any other shape.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub use crate::types::ClipItem;

pub mod formats {
    /// UTF-8 text.
    pub const TEXT: &str = "text/plain";
    pub const HTML: &str = "text/html";
    pub const PNG: &str = "image/png";
    /// Files, as `super::Files`.
    pub const FILES: &str = "be3/files";
    pub const TEXT_FRAGMENT: &str = "be3/text-fragment";
    pub const BLOCKS: &str = "be3/blocks";
    pub const CANVAS_ITEMS: &str = "be3/canvas-items";

    /// Whether `format` is one of be3's own, which other programs see only
    /// where the platform carries custom formats.
    pub fn is_be3(format: &str) -> bool {
        format.starts_with("be3/")
    }
}

impl ClipItem {
    pub fn new(format: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            format: format.into(),
            data,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new(formats::TEXT, text.into().into_bytes())
    }

    pub fn png(png: Vec<u8>) -> Self {
        Self::new(formats::PNG, png)
    }

    /// The text of a `text/plain` item; invalid UTF-8 is replaced.
    pub fn as_text(&self) -> Option<String> {
        (self.format == formats::TEXT).then(|| String::from_utf8_lossy(&self.data).into_owned())
    }

    /// The be3 format `F` this item holds, if it holds it and decodes.
    pub fn decode<F: ClipFormat>(&self) -> Option<F> {
        F::decode(self)
    }
}

/// A be3 format: a serde type with its format id.
pub trait ClipFormat: Serialize + DeserializeOwned {
    const FORMAT: &'static str;

    fn encode(&self) -> ClipItem {
        ClipItem::new(
            Self::FORMAT,
            postcard::to_stdvec(self).expect("a clip format encodes"),
        )
    }

    fn decode(item: &ClipItem) -> Option<Self> {
        (item.format == Self::FORMAT)
            .then(|| postcard::from_bytes(&item.data).ok())
            .flatten()
    }
}

/// How a text fragment was selected, for the receiver to decide what it means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextShape {
    /// A run of characters per cursor.
    #[default]
    Chars,
    /// Whole lines per cursor, each chunk ending in its line break: a text
    /// editor pastes them on their own line.
    Lines,
}

/// `be3/text-fragment`: one chunk of raw bytes per cursor, which need not be
/// UTF-8, and their shape.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFragment {
    pub chunks: Vec<Vec<u8>>,
    pub shape: TextShape,
}

impl ClipFormat for TextFragment {
    const FORMAT: &'static str = formats::TEXT_FRAGMENT;
}

impl TextFragment {
    pub fn chars(text: impl Into<Vec<u8>>) -> Self {
        Self {
            chunks: vec![text.into()],
            shape: TextShape::Chars,
        }
    }

    /// The chunks joined with line breaks, a chunk that ends in one needing
    /// none.
    pub fn plain_bytes(&self) -> Vec<u8> {
        let mut joined = Vec::new();
        for (index, chunk) in self.chunks.iter().enumerate() {
            if index > 0 && joined.last() != Some(&b'\n') {
                joined.push(b'\n');
            }
            joined.extend_from_slice(chunk);
        }
        joined
    }

    /// `plain_bytes` decoded lossily: the `text/plain` beside the fragment.
    pub fn plain_text(&self) -> String {
        String::from_utf8_lossy(&self.plain_bytes()).into_owned()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(Vec::is_empty)
    }

    /// The fragment and its plain text, richest first.
    pub fn items(&self) -> Vec<ClipItem> {
        vec![self.encode(), ClipItem::text(self.plain_text())]
    }

    /// The fragment of `item`: the fragment itself, or plain text as one run
    /// of characters.
    pub fn of(item: &ClipItem) -> Option<Self> {
        item.decode::<Self>()
            .or_else(|| (item.format == formats::TEXT).then(|| Self::chars(item.data.clone())))
    }
}

/// One block in `be3/blocks`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipBlock {
    pub id: Uuid,
    pub organization: Option<Uuid>,
    pub block_type: Uuid,
    pub title: String,
}

/// `be3/blocks`: references to blocks. Pasting adds references; whether a
/// paste moves a block is the host's cut record, never the clip's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocks {
    pub blocks: Vec<ClipBlock>,
}

impl ClipFormat for Blocks {
    const FORMAT: &'static str = formats::BLOCKS;
}

/// `be3/canvas-items`: canvas entities in the canvas's own encoding, from the
/// canvas block `canvas`. Raster layers are not embedded: a paste reads their
/// pixels from `canvas` by entity id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasItems {
    pub canvas: Uuid,
    pub entities: Vec<u8>,
}

impl ClipFormat for CanvasItems {
    const FORMAT: &'static str = formats::CANVAS_ITEMS;
}

/// One file in `be3/files`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipFile {
    pub name: String,
    pub data: Vec<u8>,
}

/// `be3/files`: files, as the platform's file lists carry them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Files {
    pub files: Vec<ClipFile>,
}

impl ClipFormat for Files {
    const FORMAT: &'static str = formats::FILES;
}
