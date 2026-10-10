use std::collections::{BTreeMap, HashSet};

use be_model::{Anchor, BlockRef, Change, Document, Edit, Item, List, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root};

const ROWS: Uuid = Uuid::from_u128(0x7a1c_52e0_9b4d_4f36_8d2a_6c1e_0f93_b5d7);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default)]
pub struct DatabaseColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(default)]
pub struct RowRef {
    #[serde(with = "be_model::references::block_ref")]
    pub database: Uuid,
    pub row: Uuid,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum DatabaseValue {
    String(String),
    Number(f64),
    Enum(Uuid),
    Block(#[serde(with = "be_model::references::block_ref")] Uuid),
    Boolean(bool),
    Color(DatabaseColor),
    Datetime(i64),
    Relation(RowRef),
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Database {
    pub schema: Option<Uuid>,
    pub prefix: String,
    pub rows: List<DatabaseRow>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DatabaseRow {
    pub values: Map<Uuid, DatabaseValue>,
    pub page: Option<BlockRef>,
}

impl DatabaseRow {
    pub fn values(&self) -> &BTreeMap<Uuid, DatabaseValue> {
        &self.values
    }

    pub fn value(&self, field: Uuid) -> Option<&DatabaseValue> {
        self.values.get(&field)
    }

    pub fn page(&self) -> Option<Uuid> {
        self.page.map(|page| page.0)
    }
}

const SHORT_ID_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SHORT_ID_LEN: usize = 6;

pub fn short_id(prefix: &str, row: Uuid) -> String {
    let mut bits = row.as_u128();
    let mut code = [0u8; SHORT_ID_LEN];
    for slot in code.iter_mut().rev() {
        *slot = SHORT_ID_ALPHABET[(bits & 31) as usize];
        bits >>= 5;
    }
    let code = String::from_utf8_lossy(&code).into_owned();
    match prefix.trim() {
        "" => code,
        prefix => format!("{prefix}-{code}"),
    }
}

fn short_code(text: &str) -> String {
    let code = text.rsplit('-').next().unwrap_or(text);
    code.trim()
        .chars()
        .map(|character| match character.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

impl Database {
    pub fn with_schema(schema: Uuid) -> Self {
        Self {
            schema: Some(schema),
            ..Self::default()
        }
    }

    pub fn set_schema(schema: Uuid) -> Edit {
        Self::SCHEMA.set(ObjectId::ROOT, &Some(schema)).into()
    }

    pub fn set_prefix(prefix: impl Into<String>) -> Edit {
        Self::PREFIX
            .set(ObjectId::ROOT, &prefix.into().trim().to_owned())
            .into()
    }

    pub fn short_id(&self, row: ObjectId) -> String {
        short_id(&self.prefix, row.as_uuid())
    }

    pub fn find_short_id(&self, text: &str) -> Option<ObjectId> {
        let wanted = short_code(text);
        if wanted.len() != SHORT_ID_LEN {
            return None;
        }
        self.rows
            .iter()
            .map(|row| row.id)
            .find(|id| short_id("", id.as_uuid()) == wanted)
    }

    pub fn title(&self, row: ObjectId, title_field: Option<Uuid>) -> String {
        let title = title_field
            .and_then(|field| self.rows.get(row)?.value(field).cloned())
            .and_then(|value| match value {
                DatabaseValue::String(text) if !text.trim().is_empty() => Some(text),
                _ => None,
            });
        title.unwrap_or_else(|| self.short_id(row))
    }

    pub fn index_of(&self, row: ObjectId) -> Option<usize> {
        self.rows.iter().position(|item| item.id == row)
    }

    pub fn add_row(values: impl IntoIterator<Item = (Uuid, DatabaseValue)>) -> (ObjectId, Edit) {
        let row = DatabaseRow {
            values: values.into_iter().collect(),
            page: None,
        };
        let (id, change) = Self::ROWS.insert(ObjectId::ROOT, Anchor::End, &row);
        (id, change.into())
    }

    pub fn set_value(row: ObjectId, field: Uuid, value: Option<&DatabaseValue>) -> Edit {
        DatabaseRow::VALUES.put(row, &field, value).into()
    }

    pub fn set_page(row: ObjectId, page: Option<Uuid>) -> Edit {
        DatabaseRow::PAGE.set(row, &page.map(BlockRef)).into()
    }

    pub fn remove_row(row: ObjectId) -> Edit {
        Change::remove(row).into()
    }

    pub fn set_cell(&self, row: usize, field: Uuid, value: Option<DatabaseValue>) -> Edit {
        if let Some(existing) = self.rows.get_index(row) {
            let mut changes = vec![DatabaseRow::VALUES.put(existing.id, &field, value.as_ref())];
            if value.is_none() {
                let empty_after = |index: usize, item: &Item<DatabaseRow>| {
                    item.values.keys().all(|key| index == row && *key == field)
                };
                changes.extend(
                    self.rows
                        .iter()
                        .enumerate()
                        .rev()
                        .take_while(|(index, item)| empty_after(*index, item))
                        .map(|(_, item)| Change::remove_if_blank::<DatabaseRow>(item.id)),
                );
            }
            return changes.into_iter().collect();
        }
        let Some(value) = value else {
            return Edit::default();
        };
        let mut changes = Vec::new();
        let mut last = self.rows.last_id();
        for index in self.rows.len()..=row {
            let id = self.row_id(index);
            let anchor = last.map_or(Anchor::Start, Anchor::After);
            changes.push(Self::ROWS.insert_as(id, ObjectId::ROOT, anchor, &DatabaseRow::default()));
            last = Some(id);
        }
        if let Some(id) = last {
            changes.push(DatabaseRow::VALUES.put(id, &field, Some(&value)));
        }
        changes.into_iter().collect()
    }

    fn row_id(&self, index: usize) -> ObjectId {
        let id = ObjectId::from_uuid(Uuid::from_u128(ROWS.as_u128().wrapping_add(index as u128)));
        if self.rows.get(id).is_some() {
            ObjectId::new()
        } else {
            id
        }
    }

    pub fn block_references(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.rows
            .iter()
            .flat_map(|row| row.values.values())
            .filter_map(|value| match value {
                DatabaseValue::Block(reference) => Some(*reference),
                _ => None,
            })
    }
}

impl Root for Database {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x0064_6174_6162_6173_652d_626c_6f63_6b01);

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let (old, new) = match change {
            ChildChange::Add(_) => return None,
            ChildChange::Delete(old) => (old, None),
            ChildChange::Replace { old, new } => (old, Some(new)),
        };
        let replaced = |value: &DatabaseValue| match value {
            DatabaseValue::Block(block) if *block == old => Some(new.map(DatabaseValue::Block)),
            DatabaseValue::Relation(link) if link.database == old => Some(new.map(|database| {
                DatabaseValue::Relation(RowRef {
                    database,
                    row: link.row,
                })
            })),
            _ => None,
        };
        Some(
            self.rows
                .iter()
                .flat_map(|row| {
                    let values = row.values.iter().filter_map(|(field, value)| {
                        let next = replaced(value)?;
                        Some(DatabaseRow::VALUES.put(row.id, field, next.as_ref()))
                    });
                    let page = (row.page() == Some(old))
                        .then(|| DatabaseRow::PAGE.set(row.id, &new.map(BlockRef)));
                    values.chain(page).collect::<Vec<_>>()
                })
                .collect(),
        )
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.schema
            .into_iter()
            .chain(self.block_references())
            .filter_map(Some)
            .filter(|reference| seen.insert(*reference))
            .collect()
    }
}

pub type DatabaseContent = Document<Database>;
