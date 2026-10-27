use std::collections::{BTreeMap, HashSet};

use be_model::{Anchor, BlockRef, Change, Document, Edit, Item, List, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::{ChildChange, Root};

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
    Links(Vec<RowRef>),
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(default)]
pub struct RowLink {
    pub field: Uuid,
    pub target: RowRef,
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
    pub links: Map<RowLink, bool>,
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

    pub fn links_of(&self, field: Uuid) -> Vec<RowRef> {
        self.links
            .iter()
            .filter(|(link, held)| **held && link.field == field)
            .map(|(link, _)| link.target)
            .collect()
    }

    pub fn get(&self, field: Uuid) -> Option<DatabaseValue> {
        let links = self.links_of(field);
        if !links.is_empty() {
            return Some(DatabaseValue::Links(links));
        }
        self.values.get(&field).cloned()
    }

    pub fn cells(&self) -> BTreeMap<Uuid, DatabaseValue> {
        let mut cells: BTreeMap<Uuid, DatabaseValue> = self
            .values
            .iter()
            .map(|(field, value)| (*field, value.clone()))
            .collect();
        for (link, held) in self.links.iter() {
            if !*held {
                continue;
            }
            let entry = cells
                .entry(link.field)
                .or_insert_with(|| DatabaseValue::Links(Vec::new()));
            if let DatabaseValue::Links(targets) = entry {
                targets.push(link.target);
            }
        }
        cells
    }

    pub fn links_to(&self, target: RowRef) -> Vec<Uuid> {
        let mut fields: Vec<Uuid> = self
            .links
            .iter()
            .filter(|(link, held)| **held && link.target == target)
            .map(|(link, _)| link.field)
            .chain(self.values.iter().filter_map(|(field, value)| {
                (*value == DatabaseValue::Relation(target)).then_some(*field)
            }))
            .collect();
        fields.sort();
        fields.dedup();
        fields
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inverse {
    pub id: Uuid,
    pub name: String,
    pub source: Uuid,
    pub field: Uuid,
    pub multiple: bool,
    pub rows: BTreeMap<Uuid, Vec<RowRef>>,
}

pub fn inverse_id(source: Uuid, field: Uuid) -> Uuid {
    Uuid::from_u128(source.as_u128().rotate_left(17) ^ field.as_u128() ^ 0x696e_7665_7273_6501)
}

pub fn inverses(
    target: Uuid,
    source: Uuid,
    database: &Database,
    schema: &crate::database_schema::DatabaseSchema,
) -> Vec<Inverse> {
    inverses_of(
        target,
        source,
        database.rows.iter().map(|row| (row.id, &row.value)),
        schema,
    )
}

pub fn inverses_of<'a>(
    target: Uuid,
    source: Uuid,
    rows: impl IntoIterator<Item = (ObjectId, &'a DatabaseRow)>,
    schema: &crate::database_schema::DatabaseSchema,
) -> Vec<Inverse> {
    let fields: Vec<_> = schema
        .fields()
        .into_iter()
        .filter(|field| {
            field.field_type == crate::database_schema::DatabaseFieldType::Relation
                && field.relation_options.database == Some(target)
                && !field.relation_options.inverse.trim().is_empty()
        })
        .collect();
    if fields.is_empty() {
        return Vec::new();
    }
    let rows: Vec<(ObjectId, &DatabaseRow)> = rows.into_iter().collect();
    fields
        .into_iter()
        .map(|field| {
            let mut linked_from: BTreeMap<Uuid, Vec<RowRef>> = BTreeMap::new();
            for (id, row) in &rows {
                let linked = match row.get(field.id) {
                    Some(DatabaseValue::Links(links)) => links,
                    Some(DatabaseValue::Relation(link)) => vec![link],
                    _ => Vec::new(),
                };
                for link in linked.into_iter().filter(|link| link.database == target) {
                    linked_from.entry(link.row).or_default().push(RowRef {
                        database: source,
                        row: id.as_uuid(),
                    });
                }
            }
            Inverse {
                id: inverse_id(source, field.id),
                name: field.relation_options.inverse.trim().to_owned(),
                source,
                field: field.id,
                multiple: field.relation_options.multiple,
                rows: linked_from,
            }
        })
        .collect()
}

impl Inverse {
    pub fn field(&self) -> crate::database_schema::DatabaseField {
        crate::database_schema::DatabaseField {
            id: self.id,
            name: self.name.clone(),
            field_type: crate::database_schema::DatabaseFieldType::Relation,
            enum_options: Vec::new(),
            number_options: Default::default(),
            block_options: Default::default(),
            relation_options: crate::database_schema::DatabaseRelationOptions {
                database: Some(self.source),
                multiple: true,
                inverse: String::new(),
            },
            default_option: None,
        }
    }

    pub fn shown_on(&self, row: &DatabaseRow, id: Uuid) -> DatabaseRow {
        let mut shown = row.clone();
        self.show_on(&mut shown, id);
        shown
    }

    pub fn show_on(&self, row: &mut DatabaseRow, id: Uuid) {
        let Some(targets) = self.rows.get(&id) else {
            return;
        };
        let links = std::mem::take(&mut row.links);
        row.links = links
            .iter()
            .map(|(link, held)| (*link, *held))
            .chain(targets.iter().map(|target| {
                (
                    RowLink {
                        field: self.id,
                        target: *target,
                    },
                    true,
                )
            }))
            .collect();
    }

    pub fn edits(&self, source: &Database, target: RowRef, wanted: &[RowRef]) -> Edit {
        let current = self.rows.get(&target.row).cloned().unwrap_or_default();
        let removed = current.iter().filter(|held| !wanted.contains(held));
        let added = wanted.iter().filter(|link| !current.contains(link));
        removed
            .map(|link| (link, false))
            .chain(added.map(|link| (link, true)))
            .filter(|(link, _)| link.database == self.source)
            .filter_map(|(link, linked)| {
                let row = source.rows.get(ObjectId::from_uuid(link.row))?;
                Some(match (linked, self.multiple) {
                    (true, true) => Database::link(row.id, self.field, target),
                    (false, true) => Database::unlink(row.id, self.field, target),
                    (true, false) => Database::set_value(
                        row.id,
                        self.field,
                        Some(&DatabaseValue::Relation(target)),
                    ),
                    (false, false) => Database::set_value(row.id, self.field, None),
                })
            })
            .flat_map(|edit| edit.0)
            .collect()
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
        match self.rows_with_short_id(text).as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }

    pub fn rows_with_short_id(&self, text: &str) -> Vec<ObjectId> {
        let wanted = short_code(text);
        if wanted.len() != SHORT_ID_LEN {
            return Vec::new();
        }
        self.rows
            .iter()
            .map(|row| row.id)
            .filter(|id| short_id("", id.as_uuid()) == wanted)
            .collect()
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
            ..DatabaseRow::default()
        };
        let (id, change) = Self::ROWS.insert(ObjectId::ROOT, Anchor::End, &row);
        (id, change.into())
    }

    pub fn set_value(row: ObjectId, field: Uuid, value: Option<&DatabaseValue>) -> Edit {
        DatabaseRow::VALUES.put(row, &field, value).into()
    }

    pub fn link(row: ObjectId, field: Uuid, target: RowRef) -> Edit {
        DatabaseRow::LINKS
            .put(row, &RowLink { field, target }, Some(&true))
            .into()
    }

    pub fn unlink(row: ObjectId, field: Uuid, target: RowRef) -> Edit {
        DatabaseRow::LINKS
            .put(row, &RowLink { field, target }, None)
            .into()
    }

    pub fn set_links(&self, row: ObjectId, field: Uuid, targets: &[RowRef]) -> Edit {
        let current = self
            .rows
            .get(row)
            .map(|row| row.links_of(field))
            .unwrap_or_default();
        current
            .iter()
            .filter(|held| !targets.contains(held))
            .map(|gone| {
                DatabaseRow::LINKS.put(
                    row,
                    &RowLink {
                        field,
                        target: *gone,
                    },
                    None,
                )
            })
            .chain(
                targets
                    .iter()
                    .filter(|wanted| !current.contains(wanted))
                    .map(|added| {
                        DatabaseRow::LINKS.put(
                            row,
                            &RowLink {
                                field,
                                target: *added,
                            },
                            Some(&true),
                        )
                    }),
            )
            .collect()
    }

    pub fn linking(&self, target: RowRef, field: Uuid) -> Vec<ObjectId> {
        self.rows
            .iter()
            .filter(|row| row.links_to(target).contains(&field))
            .map(|row| row.id)
            .collect()
    }

    pub fn set_page(row: ObjectId, page: Option<Uuid>) -> Edit {
        DatabaseRow::PAGE.set(row, &page.map(BlockRef)).into()
    }

    pub fn move_row(row: ObjectId, after: Option<ObjectId>) -> Edit {
        let anchor = after.map_or(Anchor::Start, Anchor::After);
        Self::ROWS.move_into(ObjectId::ROOT, anchor, row).into()
    }

    pub fn remove_row(row: ObjectId) -> Edit {
        Change::remove(row).into()
    }

    pub fn remove_row_and_links(&self, database: Uuid, row: ObjectId) -> Edit {
        let gone = RowRef {
            database,
            row: row.as_uuid(),
        };
        let mut changes = vec![Change::remove(row)];
        for other in self.rows.iter().filter(|other| other.id != row) {
            for field in other.links_to(gone) {
                changes.push(DatabaseRow::LINKS.put(
                    other.id,
                    &RowLink {
                        field,
                        target: gone,
                    },
                    None,
                ));
                if other.value(field) == Some(&DatabaseValue::Relation(gone)) {
                    changes.push(DatabaseRow::VALUES.put(other.id, &field, None));
                }
            }
        }
        changes.into_iter().collect()
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

    const FROZEN: bool = true;

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let (old, new) = match change {
            ChildChange::Add(_) | ChildChange::Reorder { .. } => return None,
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
                    let links = row
                        .links
                        .iter()
                        .filter(|(link, held)| **held && link.target.database == old)
                        .flat_map(|(link, _)| {
                            let removed = DatabaseRow::LINKS.put(row.id, link, None);
                            let added = new.map(|database| {
                                let moved = RowLink {
                                    field: link.field,
                                    target: RowRef {
                                        database,
                                        row: link.target.row,
                                    },
                                };
                                DatabaseRow::LINKS.put(row.id, &moved, Some(&true))
                            });
                            std::iter::once(removed).chain(added)
                        })
                        .collect::<Vec<_>>();
                    values.chain(page).chain(links).collect::<Vec<_>>()
                })
                .collect(),
        )
    }

    fn describe(&self, metadata: &mut be_block::BlockMetadata) {
        metadata.insert(
            &be_block::metadata::ROW_PREFIX,
            &self.prefix.trim().to_owned(),
        );
    }

    fn name_in(_document: &Document<Self>) -> Option<String> {
        None
    }

    fn describe_in(document: &Document<Self>, metadata: &mut be_block::BlockMetadata) {
        let prefix = document.field(ObjectId::ROOT, Self::PREFIX);
        metadata.insert(&be_block::metadata::ROW_PREFIX, &prefix.trim().to_owned());
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
