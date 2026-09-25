use be_model::{Document, Edit, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseViewSort {
    pub field_id: Uuid,
    pub direction: SortDirection,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum DatabaseViewKind {
    #[default]
    Spreadsheet,
    Kanban,
    Scatter,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct DatabaseView {
    pub database: Option<Uuid>,
    pub sort: Option<DatabaseViewSort>,
    pub kind: DatabaseViewKind,
    pub kanban_field: Option<Uuid>,
    pub scatter_x: Option<Uuid>,
    pub scatter_y: Option<Uuid>,
}

impl DatabaseView {
    pub fn of(database: Uuid) -> Self {
        Self {
            database: Some(database),
            ..Self::default()
        }
    }

    pub fn set_database(database: Uuid) -> Edit {
        Self::DATABASE.set(ObjectId::ROOT, &Some(database)).into()
    }

    pub fn set_sort(sort: Option<DatabaseViewSort>) -> Edit {
        Self::SORT.set(ObjectId::ROOT, &sort).into()
    }

    pub fn set_kind(kind: DatabaseViewKind) -> Edit {
        Self::KIND.set(ObjectId::ROOT, &kind).into()
    }

    pub fn set_kanban_field(field: Option<Uuid>) -> Edit {
        Self::KANBAN_FIELD.set(ObjectId::ROOT, &field).into()
    }

    pub fn set_scatter_x(field: Option<Uuid>) -> Edit {
        Self::SCATTER_X.set(ObjectId::ROOT, &field).into()
    }

    pub fn set_scatter_y(field: Option<Uuid>) -> Edit {
        Self::SCATTER_Y.set(ObjectId::ROOT, &field).into()
    }
}

impl Root for DatabaseView {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x0000_6461_7461_6261_7365_2d76_6965_7701);

    fn references(&self) -> Vec<Uuid> {
        self.database.into_iter().collect()
    }
}

pub type DatabaseViewContent = Document<DatabaseView>;
