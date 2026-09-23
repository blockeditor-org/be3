use std::collections::{BTreeMap, HashSet};

use be_model::{Anchor, Change, Document, Edit, Item, List, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockRef, ChildChange, Root};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum DatabaseValue {
    String(String),
    Number(f64),
    Enum(Uuid),
    Block(BlockRef),
    Boolean(bool),
    Color(DatabaseColor),
    Datetime(i64),
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Database {
    pub schema: Option<BlockRef>,
    pub rows: List<DatabaseRow>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DatabaseRow {
    pub values: Map<Uuid, DatabaseValue>,
}

impl DatabaseRow {
    pub fn values(&self) -> &BTreeMap<Uuid, DatabaseValue> {
        &self.values
    }

    pub fn value(&self, field: Uuid) -> Option<&DatabaseValue> {
        self.values.get(&field)
    }
}

impl Database {
    pub fn with_schema(schema: BlockRef) -> Self {
        Self {
            schema: Some(schema),
            rows: List::default(),
        }
    }

    pub fn set_schema(schema: BlockRef) -> Edit {
        Self::SCHEMA.set(ObjectId::ROOT, &Some(schema)).into()
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
                        .map(|(_, item)| Change::remove(item.id)),
                );
            }
            return changes.into_iter().collect();
        }
        let Some(value) = value else {
            return Edit::default();
        };
        let mut changes = Vec::new();
        for _ in self.rows.len()..row {
            let (_, change) =
                Self::ROWS.insert(ObjectId::ROOT, Anchor::End, &DatabaseRow::default());
            changes.push(change);
        }
        let filled = DatabaseRow {
            values: [(field, value)].into_iter().collect(),
        };
        let (_, change) = Self::ROWS.insert(ObjectId::ROOT, Anchor::End, &filled);
        changes.push(change);
        changes.into_iter().collect()
    }

    pub fn block_references(&self) -> impl Iterator<Item = BlockRef> + '_ {
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
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6461_7461_6261_7365_2d63_6f6e_7465_6e74);

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let (old, new) = match change {
            ChildChange::Add(_) => return None,
            ChildChange::Delete(old) => (old, None),
            ChildChange::Replace { old, new } => {
                (old, Some(DatabaseValue::Block(BlockRef::Direct(new))))
            }
        };
        let old = DatabaseValue::Block(BlockRef::Direct(old));
        Some(
            self.rows
                .iter()
                .flat_map(|row| {
                    row.values
                        .iter()
                        .filter(|(_, value)| **value == old)
                        .map(|(field, _)| DatabaseRow::VALUES.put(row.id, field, new.as_ref()))
                })
                .collect(),
        )
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.schema
            .into_iter()
            .chain(self.block_references())
            .filter_map(|reference| reference.as_direct())
            .filter(|reference| seen.insert(*reference))
            .collect()
    }
}

pub type DatabaseContent = Document<Database>;
