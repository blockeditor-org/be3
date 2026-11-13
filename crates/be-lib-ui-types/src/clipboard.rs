//! What the clipboard and drag and drop carry: a clip lists representations of
//! the same content, richest first, each a format id and its data
//! (`ClipItem`). The receiver chooses, by listing the formats it takes; it
//! gets the first the clip holds.
//!
//! Standard formats keep their MIME type and are mapped to each platform's own.
//! be3 formats are the serde types below, encoded with postcard. They are not
//! frozen (guides/frozen_formats.md): a format may change like any other shape.
//!
//! An item names blocks only in its `refs`, which its data indexes by
//! position: whoever carries it translates them without decoding the data.

use std::ops::Range;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub use uuid::Uuid;

pub use crate::types::{ClipBody, ClipItem};

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
    /// An item naming no blocks.
    pub fn new(format: impl Into<String>, data: Vec<u8>) -> Self {
        Self::of_body(format, ClipBody::from(data))
    }

    pub fn of_body(format: impl Into<String>, body: ClipBody) -> Self {
        Self {
            format: format.into(),
            data: body.data,
            refs: body.refs,
        }
    }

    pub fn into_body(self) -> ClipBody {
        ClipBody {
            data: self.data,
            refs: self.refs,
        }
    }

    /// The item as one run of bytes, refs and all, for a system clipboard
    /// that carries a be3 format as a custom type of its own.
    pub fn stored(&self) -> Vec<u8> {
        postcard::to_stdvec(&(&self.refs, &self.data)).expect("a clip item encodes")
    }

    /// An item of `format` read back from its `stored` bytes.
    pub fn from_stored(format: impl Into<String>, stored: &[u8]) -> Option<Self> {
        let (refs, data): (Vec<Uuid>, Vec<u8>) = postcard::from_bytes(stored).ok()?;
        Some(Self::of_body(format, ClipBody { data, refs }))
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

impl From<Vec<u8>> for ClipBody {
    fn from(data: Vec<u8>) -> Self {
        Self {
            data,
            refs: Vec::new(),
        }
    }
}

/// A be3 format: a serde type with its format id. A format naming blocks
/// leaves their ids out of its serde form (`#[serde(skip)]`) and lists them
/// in `refs`, in an order its data implies; `set_refs` puts them back.
pub trait ClipFormat: Serialize + DeserializeOwned {
    const FORMAT: &'static str;

    /// The blocks the value names, in the order `set_refs` takes them.
    fn refs(&self) -> Vec<Uuid> {
        Vec::new()
    }

    /// Puts back the blocks `refs` named into a value just decoded; `None`
    /// when they do not fit it.
    fn set_refs(&mut self, refs: &[Uuid]) -> Option<()> {
        refs.is_empty().then_some(())
    }

    fn encode(&self) -> ClipItem {
        ClipItem::of_body(
            Self::FORMAT,
            ClipBody {
                data: postcard::to_stdvec(self).expect("a clip format encodes"),
                refs: self.refs(),
            },
        )
    }

    fn decode(item: &ClipItem) -> Option<Self> {
        if item.format != Self::FORMAT {
            return None;
        }
        let mut value: Self = postcard::from_bytes(&item.data).ok()?;
        value.set_refs(&item.refs)?;
        Some(value)
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

/// `be3/text-fragment`: one chunk per cursor, which need not be UTF-8, and
/// their shape. A rich text's marks travel in its chunks, keyed by `schema`:
/// a receiver of another schema, or none, takes the bytes alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFragment {
    pub chunks: Vec<TextChunk>,
    pub shape: TextShape,
    /// The schema the marks' keys belong to (`Schema::name` in
    /// beui-text-editor-core); `None` when they carry none.
    pub schema: Option<String>,
}

/// One cursor's bytes and the marks over them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextChunk {
    pub bytes: Vec<u8>,
    pub marks: Vec<TextMark>,
}

/// A mark over `range`, bytes within its chunk. An inline mark styles its
/// bytes; a block mark starts at the start of a line and gives that line its
/// props.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextMark {
    pub range: Range<usize>,
    pub block: bool,
    pub key: String,
    pub value: MarkValue,
}

/// A mark's value. A block it names (an embed) travels in the item's refs,
/// in the order the chunks and their marks list them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkValue {
    Bool(bool),
    Int(i64),
    Text(String),
    Block {
        #[serde(skip)]
        id: Uuid,
    },
}

impl ClipFormat for TextFragment {
    const FORMAT: &'static str = formats::TEXT_FRAGMENT;

    fn refs(&self) -> Vec<Uuid> {
        self.blocks().map(|id| *id).collect()
    }

    fn set_refs(&mut self, refs: &[Uuid]) -> Option<()> {
        let mut refs = refs.iter();
        for id in self.blocks_mut() {
            *id = *refs.next()?;
        }
        refs.next().is_none().then_some(())
    }
}

impl TextChunk {
    pub fn plain(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            marks: Vec::new(),
        }
    }
}

impl TextFragment {
    pub fn chars(text: impl Into<Vec<u8>>) -> Self {
        Self {
            chunks: vec![TextChunk::plain(text)],
            shape: TextShape::Chars,
            schema: None,
        }
    }

    /// Chunks of bytes alone, without marks.
    pub fn plain(chunks: impl IntoIterator<Item = Vec<u8>>, shape: TextShape) -> Self {
        Self {
            chunks: chunks.into_iter().map(TextChunk::plain).collect(),
            shape,
            schema: None,
        }
    }

    fn blocks(&self) -> impl Iterator<Item = &Uuid> {
        self.chunks
            .iter()
            .flat_map(|chunk| &chunk.marks)
            .filter_map(|mark| match &mark.value {
                MarkValue::Block { id } => Some(id),
                _ => None,
            })
    }

    fn blocks_mut(&mut self) -> impl Iterator<Item = &mut Uuid> {
        self.chunks
            .iter_mut()
            .flat_map(|chunk| &mut chunk.marks)
            .filter_map(|mark| match &mut mark.value {
                MarkValue::Block { id } => Some(id),
                _ => None,
            })
    }

    /// The chunks joined into one with line breaks, a chunk that ends in one
    /// needing none, their marks moved along.
    pub fn joined(&self) -> TextChunk {
        let mut joined = TextChunk::default();
        for (index, chunk) in self.chunks.iter().enumerate() {
            if index > 0 && joined.bytes.last() != Some(&b'\n') {
                joined.bytes.push(b'\n');
            }
            let at = joined.bytes.len();
            joined.bytes.extend_from_slice(&chunk.bytes);
            joined.marks.extend(chunk.marks.iter().map(|mark| TextMark {
                range: mark.range.start + at..mark.range.end + at,
                ..mark.clone()
            }));
        }
        joined
    }

    /// The chunks joined with line breaks, a chunk that ends in one needing
    /// none.
    pub fn plain_bytes(&self) -> Vec<u8> {
        self.joined().bytes
    }

    /// `plain_bytes` decoded lossily: the `text/plain` beside the fragment.
    pub fn plain_text(&self) -> String {
        String::from_utf8_lossy(&self.plain_bytes()).into_owned()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(|chunk| chunk.bytes.is_empty())
    }

    /// The fragment, `html` written from its marks when there is one, and its
    /// plain text, richest first.
    pub fn items(&self, html: Option<String>) -> Vec<ClipItem> {
        std::iter::once(self.encode())
            .chain(html.map(|html| ClipItem::new(formats::HTML, html.into_bytes())))
            .chain([ClipItem::text(self.plain_text())])
            .collect()
    }

    /// The fragment of `item`: the fragment itself, or plain text as one run
    /// of characters.
    pub fn of(item: &ClipItem) -> Option<Self> {
        item.decode::<Self>()
            .or_else(|| (item.format == formats::TEXT).then(|| Self::chars(item.data.clone())))
    }
}

/// One block in `be3/blocks`. Its id travels in the item's refs, one per
/// block in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipBlock {
    #[serde(skip)]
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

    fn refs(&self) -> Vec<Uuid> {
        self.blocks.iter().map(|block| block.id).collect()
    }

    fn set_refs(&mut self, refs: &[Uuid]) -> Option<()> {
        if refs.len() != self.blocks.len() {
            return None;
        }
        for (block, id) in self.blocks.iter_mut().zip(refs) {
            block.id = *id;
        }
        Some(())
    }
}

/// `be3/canvas-items`: canvas entities in the canvas's own encoding, from the
/// canvas block `canvas`. Raster layers are not embedded: a paste reads their
/// pixels from `canvas` by entity id. `blocks` are every block the entities
/// name (shown blocks, component schemas, the blocks and databases component
/// values name), in the order the canvas walks them; `entities` holds nil in
/// their place. `canvas` and `blocks` travel in the refs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasItems {
    #[serde(skip)]
    pub canvas: Uuid,
    #[serde(skip)]
    pub blocks: Vec<Uuid>,
    pub entities: Vec<u8>,
}

impl ClipFormat for CanvasItems {
    const FORMAT: &'static str = formats::CANVAS_ITEMS;

    fn refs(&self) -> Vec<Uuid> {
        std::iter::once(self.canvas)
            .chain(self.blocks.iter().copied())
            .collect()
    }

    fn set_refs(&mut self, refs: &[Uuid]) -> Option<()> {
        let (canvas, blocks) = refs.split_first()?;
        self.canvas = *canvas;
        self.blocks = blocks.to_vec();
        Some(())
    }
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

#[cfg(test)]
mod tests;
