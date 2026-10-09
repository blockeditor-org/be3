use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    Items, Kept, Kind, Malformed, Object, ObjectId, Objects, Place, Sequence, Shape, Tree, Value,
    grid::Cells,
    latest::{Stamp, Stamped},
};

pub use ciborium::Value as Stored;

pub const BLOCK_REF: u64 = 0x6265_3301;
pub const BLOB_REF: u64 = 0x6265_3302;
pub const CRITICAL: u64 = 0x6265_3303;
const UUID: u64 = 37;

const KIND: &str = "$kind";
const ID: &str = "$id";

pub(crate) fn encode<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    match ciborium::into_writer(value, &mut out) {
        Ok(()) => out,
        Err(_) => Vec::new(),
    }
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    ciborium::from_reader(bytes).ok()
}

pub(crate) fn is_critical(bytes: &[u8]) -> bool {
    matches!(decode::<Stored>(bytes), Some(Stored::Tag(CRITICAL, _)))
}

fn text(value: &str) -> Stored {
    Stored::Text(value.to_owned())
}

fn uuid(id: Uuid) -> Stored {
    Stored::Tag(UUID, Box::new(Stored::Bytes(id.as_bytes().to_vec())))
}

fn as_uuid(value: &Stored) -> Option<Uuid> {
    match value {
        Stored::Tag(UUID, inner) => Uuid::from_slice(inner.as_bytes()?).ok(),
        _ => None,
    }
}

fn int(value: &Stored) -> Option<i128> {
    value.as_integer().map(i128::from)
}

fn sorted(mut entries: Vec<(String, Stored)>) -> Stored {
    entries.sort_by(|(a, _), (b, _)| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    Stored::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Stored::Text(key), value))
            .collect(),
    )
}

pub(crate) fn document(tree: &Tree, kind: &Kind) -> Stored {
    sorted(vec![
        ("format".to_owned(), Stored::from(kind.format)),
        ("root".to_owned(), object(tree, ObjectId::ROOT, kind, false)),
    ])
}

pub(crate) fn save(tree: &Tree, kind: &Kind) -> Vec<u8> {
    encode(&document(tree, kind))
}

fn object(tree: &Tree, id: ObjectId, kind: &Kind, with_id: bool) -> Stored {
    let Some(held) = tree.object(id) else {
        return sorted(vec![(KIND.to_owned(), text(kind.name))]);
    };
    let mut entries = vec![(
        KIND.to_owned(),
        text(held.kept.kind.as_deref().unwrap_or(kind.name)),
    )];
    if with_id {
        entries.push((ID.to_owned(), uuid(id.as_uuid())));
    }
    let mut written = BTreeSet::new();
    if held.kept.kind.is_none() {
        let blank = (kind.blank)();
        for (index, property) in kind.properties.iter().enumerate() {
            let Some(value) = held.fields.get(index) else {
                continue;
            };
            let mut stored = field(tree, value, &property.shape);
            if property.critical && blank.get(index) != Some(value) {
                stored = Stored::Tag(CRITICAL, Box::new(stored));
            }
            written.insert(property.name);
            entries.push((property.name.to_owned(), stored));
        }
    }
    for (name, bytes) in &held.kept.properties {
        if written.contains(name.as_str()) {
            continue;
        }
        if let Some(value) = decode::<Stored>(bytes) {
            entries.push((name.clone(), value));
        }
    }
    sorted(entries)
}

fn field(tree: &Tree, value: &Value, shape: &Shape) -> Stored {
    let raw = |bytes: &[u8]| decode::<Stored>(bytes).unwrap_or(Stored::Null);
    match value {
        Value::Register(bytes) => raw(bytes),
        Value::Count(count) => Stored::from(*count),
        Value::Text(sequence) => Stored::Bytes(sequence.items()),
        Value::List(items) => {
            let Shape::List(child) = shape else {
                return Stored::Array(Vec::new());
            };
            let child = child();
            Stored::Array(
                items
                    .ids()
                    .into_iter()
                    .map(|id| object(tree, id, &child, true))
                    .collect(),
            )
        }
        Value::Map(entries) => Stored::Array(
            entries
                .iter()
                .map(|(key, value)| Stored::Array(vec![raw(key), raw(value)]))
                .collect(),
        ),
        Value::Grid(cells) => cells.to_stored(),
        Value::Latest(entries) => Stored::Array(
            entries
                .iter()
                .map(|(key, stamped)| {
                    let mut entry = vec![
                        raw(key),
                        Stored::from(stamped.stamp.time),
                        uuid(stamped.stamp.origin),
                    ];
                    entry.extend(stamped.value.as_deref().map(raw));
                    Stored::Array(entry)
                })
                .collect(),
        ),
    }
}

pub(crate) fn load(bytes: &[u8], kind: &Kind) -> Result<Tree, Malformed> {
    let envelope: Stored = decode(bytes).ok_or(Malformed)?;
    let mut entries = properties(envelope).ok_or(Malformed)?;
    let format = entries
        .remove("format")
        .as_ref()
        .and_then(int)
        .and_then(|format| u32::try_from(format).ok())
        .ok_or(Malformed)?;
    let mut root = entries.remove("root").ok_or(Malformed)?;
    if format > kind.format {
        return Err(Malformed);
    }
    if format < kind.format {
        (kind.migrate)(format, &mut root);
    }
    let mut loading = Loading::default();
    let root = properties(root).ok_or(Malformed)?;
    loading.object(root, kind, ObjectId::ROOT, None, true);
    Ok(Tree::from_objects(loading.objects))
}

fn properties(value: Stored) -> Option<BTreeMap<String, Stored>> {
    let Stored::Map(entries) = value else {
        return None;
    };
    Some(
        entries
            .into_iter()
            .filter_map(|(key, value)| Some((key.into_text().ok()?, value)))
            .collect(),
    )
}

fn uncritical(value: Stored) -> Stored {
    match value {
        Stored::Tag(CRITICAL, inner) => *inner,
        other => other,
    }
}

#[derive(Default)]
struct Loading {
    objects: Objects,
}

impl Loading {
    fn object(
        &mut self,
        mut entries: BTreeMap<String, Stored>,
        kind: &Kind,
        id: ObjectId,
        parent: Option<Place>,
        root: bool,
    ) {
        let found = entries.remove(KIND).and_then(|kind| kind.into_text().ok());
        entries.remove(ID);
        let mut fields = (kind.blank)();
        let mut kept = Kept::default();
        let foreign = found.filter(|found| !root && found != kind.name);
        if foreign.is_some() {
            kept.kind = foreign;
        } else {
            for (index, property) in kind.properties.iter().enumerate() {
                let mut found = entries.remove(property.name);
                for alias in &property.aliases {
                    let old = entries.remove(*alias);
                    found = found.or(old);
                }
                for (name, convert) in &property.from_old {
                    let old = entries.remove(*name);
                    if found.is_none() {
                        found = old.map(uncritical).and_then(|old| convert(&old));
                    }
                }
                let Some(found) = found.map(uncritical) else {
                    continue;
                };
                let Ok(field) = u16::try_from(index) else {
                    continue;
                };
                let place = Place { object: id, field };
                if let Some(value) = self.field(found, &property.shape, place) {
                    fields[index] = value;
                }
            }
        }
        kept.properties = entries
            .into_iter()
            .map(|(name, value)| (name, encode(&value)))
            .collect();
        self.objects.insert(
            id,
            Object {
                parent,
                fields,
                kept,
            },
        );
    }

    fn field(&mut self, found: Stored, shape: &Shape, place: Place) -> Option<Value> {
        Some(match shape {
            Shape::Register { .. } => Value::Register(encode(&found)),
            Shape::Count => {
                Value::Count(int(&found)?.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
            }
            Shape::Text => Value::Text(Sequence::from_items(found.into_bytes().ok()?)),
            Shape::List(child) => {
                let child = child();
                let mut ids = Vec::new();
                for item in found.into_array().ok()? {
                    let Some(entries) = properties(item) else {
                        continue;
                    };
                    let id = entries
                        .get(ID)
                        .and_then(as_uuid)
                        .map(ObjectId::from_uuid)
                        .filter(|id| *id != ObjectId::ROOT && !self.objects.contains_key(id))
                        .unwrap_or_else(|| self.fresh());
                    self.objects
                        .insert(id, Object::new(Some(place), Vec::new()));
                    self.object(entries, &child, id, Some(place), false);
                    ids.push(id);
                }
                Value::List(Items::from_ids(ids))
            }
            Shape::Map => Value::Map(
                found
                    .into_array()
                    .ok()?
                    .into_iter()
                    .filter_map(|entry| {
                        let mut pair = entry.into_array().ok()?.into_iter();
                        let key = pair.next()?;
                        let value = pair.next()?;
                        Some((encode(&key), encode(&value)))
                    })
                    .collect(),
            ),
            Shape::Grid => Value::Grid(Cells::from_stored(found)?),
            Shape::Latest => Value::Latest(
                found
                    .into_array()
                    .ok()?
                    .into_iter()
                    .filter_map(|entry| {
                        let mut entry = entry.into_array().ok()?.into_iter();
                        let key = encode(&entry.next()?);
                        let time = u64::try_from(int(&entry.next()?)?).ok()?;
                        let origin = as_uuid(&entry.next()?)?;
                        let value = entry.next().map(|value| encode(&value));
                        Some((
                            key,
                            Stamped {
                                stamp: Stamp { time, origin },
                                value,
                            },
                        ))
                    })
                    .collect(),
            ),
        })
    }

    fn fresh(&self) -> ObjectId {
        loop {
            let id = ObjectId::new();
            if !self.objects.contains_key(&id) {
                return id;
            }
        }
    }
}

pub fn diagnostic(value: &Stored) -> String {
    let mut out = String::new();
    write_diagnostic(value, &mut out);
    out
}

fn write_diagnostic(value: &Stored, out: &mut String) {
    match value {
        Stored::Integer(integer) => {
            let _ = write!(out, "{}", i128::from(*integer));
        }
        Stored::Bytes(bytes) => {
            out.push_str("h'");
            for byte in bytes {
                let _ = write!(out, "{byte:02x}");
            }
            out.push('\'');
        }
        Stored::Float(float) => {
            let _ = write!(out, "{float:?}");
        }
        Stored::Text(text) => {
            let _ = write!(out, "{text:?}");
        }
        Stored::Bool(bool) => {
            let _ = write!(out, "{bool}");
        }
        Stored::Null => out.push_str("null"),
        Stored::Tag(tag, inner) => {
            let _ = write!(out, "{tag}(");
            write_diagnostic(inner, out);
            out.push(')');
        }
        Stored::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_diagnostic(item, out);
            }
            out.push(']');
        }
        Stored::Map(entries) => {
            out.push('{');
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_diagnostic(key, out);
                out.push_str(": ");
                write_diagnostic(value, out);
            }
            out.push('}');
        }
        _ => out.push_str("undefined"),
    }
}
