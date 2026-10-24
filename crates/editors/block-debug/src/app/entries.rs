use std::collections::HashSet;
use std::fmt::Write as _;

use block_editor_beui::be_block::be_model::{
    BLOB_REF, BLOCK_REF, CRITICAL, Stored, from_cbor, to_cbor,
};
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{BlockInfo, BlockParent, RawContent};
use uuid::Uuid;

const UUID_TAG: u64 = 37;
const SHOWN_BYTES: usize = 32;
const OPEN_DEPTH: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) children: Vec<Entry>,
    pub(crate) at: Option<Vec<usize>>,
}

impl Entry {
    fn leaf(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            children: Vec::new(),
            at: None,
        }
    }

    fn editable(mut self, at: Vec<usize>) -> Self {
        self.at = Some(at);
        self
    }

    fn branch(key: impl Into<String>, value: impl Into<String>, children: Vec<Entry>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            children,
            at: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) path: String,
    pub(crate) label: String,
    pub(crate) depth: usize,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) value: String,
    pub(crate) at: Option<Vec<usize>>,
}

fn type_name(types: &dyn BlockTypes, block_type: Uuid) -> String {
    match types.display_name(block_type) {
        Some(name) => format!("{name} ({block_type})"),
        None => block_type.to_string(),
    }
}

pub(crate) fn block(info: Option<&BlockInfo>, types: &dyn BlockTypes) -> Entry {
    let Some(info) = info else {
        return Entry::leaf("block", "loading");
    };
    let parent = match info.parent {
        Some(BlockParent::Root) => "root".to_owned(),
        Some(BlockParent::Detached) => "detached".to_owned(),
        Some(BlockParent::Block(parent)) => parent.to_string(),
        None => "unknown".to_owned(),
    };
    let references = info
        .references
        .iter()
        .enumerate()
        .map(|(index, reference)| Entry::leaf(index.to_string(), reference.to_string()))
        .collect::<Vec<_>>();
    let mut children = vec![
        Entry::leaf("id", info.id.to_string()),
        Entry::leaf("type", type_name(types, info.block_type)),
        Entry::leaf(
            "author",
            info.author
                .map_or_else(|| "unknown".to_owned(), |author| author.to_string()),
        ),
        Entry::leaf("parent", parent),
        Entry::leaf("name", info.name().unwrap_or_default()),
        Entry::leaf("named by hand", info.named_by_hand().to_string()),
        Entry::leaf("access", info.access.label()),
        Entry::branch("references", format!("[{}]", references.len()), references),
    ];
    if let Some(artifact) = info.artifact() {
        children.push(Entry::leaf(
            "artifact of",
            type_name(types, artifact.source_type),
        ));
    }
    Entry::branch("block", String::new(), children)
}

pub(crate) fn content(content: Option<&RawContent>, types: &dyn BlockTypes) -> Entry {
    let Some(content) = content else {
        return Entry::leaf("content", "loading");
    };
    let mut children = vec![
        Entry::leaf("type", type_name(types, content.content_type)),
        Entry::leaf("size", format!("{} bytes", content.bytes.len())),
    ];
    match from_cbor::<Stored>(&content.bytes) {
        Some(Stored::Map(fields)) => children.extend(
            fields
                .iter()
                .enumerate()
                .map(|(index, (key, value))| stored(&stored_key(key), value, Some(vec![index]))),
        ),
        Some(document) => children.push(stored("document", &document, Some(Vec::new()))),
        None => children.push(Entry::leaf("bytes", hex(&content.bytes))),
    }
    Entry::branch("content", String::new(), children)
}

fn stored_key(key: &Stored) -> String {
    match key {
        Stored::Text(text) => text.clone(),
        other => scalar(other).unwrap_or_else(|| "key".to_owned()),
    }
}

fn stored(key: &str, value: &Stored, at: Option<Vec<usize>>) -> Entry {
    let within = |index: usize| {
        at.clone().map(|mut at| {
            at.push(index);
            at
        })
    };
    match value {
        Stored::Map(fields) => Entry::branch(
            key,
            format!("{{{}}}", fields.len()),
            fields
                .iter()
                .enumerate()
                .map(|(index, (key, value))| stored(&stored_key(key), value, within(index)))
                .collect(),
        ),
        Stored::Array(items) => Entry::branch(
            key,
            format!("[{}]", items.len()),
            items
                .iter()
                .enumerate()
                .map(|(index, item)| stored(&index.to_string(), item, within(index)))
                .collect(),
        ),
        Stored::Tag(BLOCK_REF, inner) => tagged(key, "block", inner),
        Stored::Tag(BLOB_REF, inner) => tagged(key, "blob", inner),
        Stored::Tag(CRITICAL, inner) => tagged(key, "critical", inner),
        Stored::Tag(tag, inner) => match scalar(value) {
            Some(shown) => Entry::leaf(key, shown),
            None => tagged(key, &format!("tag {tag}"), inner),
        },
        Stored::Null
        | Stored::Bool(_)
        | Stored::Integer(_)
        | Stored::Float(_)
        | Stored::Text(_) => {
            let leaf = Entry::leaf(key, scalar(value).unwrap_or_default());
            match at {
                Some(at) => leaf.editable(at),
                None => leaf,
            }
        }
        other => Entry::leaf(key, scalar(other).unwrap_or_default()),
    }
}

fn tagged(key: &str, tag: &str, inner: &Stored) -> Entry {
    match scalar(inner) {
        Some(shown) => Entry::leaf(key, format!("{tag} {shown}")),
        None => Entry::branch(key, tag, vec![stored("value", inner, None)]),
    }
}

fn scalar(value: &Stored) -> Option<String> {
    Some(match value {
        Stored::Null => "null".to_owned(),
        Stored::Bool(value) => value.to_string(),
        Stored::Integer(value) => i128::from(*value).to_string(),
        Stored::Float(value) => format!("{value:?}"),
        Stored::Text(value) => format!("{value:?}"),
        Stored::Bytes(bytes) => hex(bytes),
        Stored::Tag(UUID_TAG, inner) => {
            let bytes = inner.as_bytes()?;
            Uuid::from_slice(bytes).ok()?.to_string()
        }
        _ => return None,
    })
}

fn hex(bytes: &[u8]) -> String {
    let mut shown = String::new();
    for byte in bytes.iter().take(SHOWN_BYTES) {
        let _ = write!(shown, "{byte:02x}");
    }
    if bytes.len() > SHOWN_BYTES {
        shown.push('…');
    }
    format!("{} bytes {shown}", bytes.len())
}

pub(crate) fn rows(roots: &[Entry], toggled: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    for root in roots {
        push(&mut rows, toggled, root.key.clone(), root, 0);
    }
    rows
}

fn push(rows: &mut Vec<Row>, toggled: &HashSet<String>, path: String, entry: &Entry, depth: usize) {
    let expandable = !entry.children.is_empty();
    let expanded = expandable && toggled.contains(&path) != (depth < OPEN_DEPTH);
    let label = match entry.value.is_empty() {
        true => entry.key.clone(),
        false => format!("{} {}", entry.key, entry.value),
    };
    rows.push(Row {
        path: path.clone(),
        label,
        depth,
        expandable,
        expanded,
        value: entry.value.clone(),
        at: entry.at.clone(),
    });
    if !expanded {
        return;
    }
    for (index, child) in entry.children.iter().enumerate() {
        push(rows, toggled, format!("{path}.{index}"), child, depth + 1);
    }
}

pub(crate) fn replaced(bytes: &[u8], at: &[usize], typed: &str) -> Option<Vec<u8>> {
    let mut document = from_cbor::<Stored>(bytes)?;
    let mut held = &mut document;
    for &index in at {
        held = match held {
            Stored::Map(fields) => &mut fields.get_mut(index)?.1,
            Stored::Array(items) => items.get_mut(index)?,
            _ => return None,
        };
    }
    *held = literal(typed);
    Some(to_cbor(&document))
}

fn literal(typed: &str) -> Stored {
    match typed {
        "null" => return Stored::Null,
        "true" => return Stored::Bool(true),
        "false" => return Stored::Bool(false),
        _ => {}
    }
    if let Some(integer) = typed
        .parse::<i128>()
        .ok()
        .and_then(|value| value.try_into().ok())
    {
        return Stored::Integer(integer);
    }
    if let Ok(float) = typed.parse::<f64>() {
        return Stored::Float(float);
    }
    let quoted = typed
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .and_then(unescape);
    Stored::Text(quoted.unwrap_or_else(|| typed.to_owned()))
}

fn unescape(quoted: &str) -> Option<String> {
    let mut text = String::new();
    let mut chars = quoted.chars();
    while let Some(next) = chars.next() {
        if next == '"' {
            return None;
        }
        if next != '\\' {
            text.push(next);
            continue;
        }
        text.push(match chars.next()? {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            'u' => {
                let rest = chars.as_str().strip_prefix('{')?;
                let (hex, after) = rest.split_once('}')?;
                chars = after.chars();
                char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
            }
            other => other,
        });
    }
    Some(text)
}
