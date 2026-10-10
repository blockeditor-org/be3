use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::database::{DatabaseRow, DatabaseValue};

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
    List,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum FilterCondition {
    #[default]
    IsSet,
    IsEmpty,
    Is(DatabaseValue),
    IsNot(DatabaseValue),
    Contains(String),
}

impl FilterCondition {
    pub fn matches(&self, value: Option<&DatabaseValue>) -> bool {
        match self {
            Self::IsSet => value.is_some(),
            Self::IsEmpty => value.is_none(),
            Self::Is(wanted) => value == Some(wanted),
            Self::IsNot(unwanted) => value != Some(unwanted),
            Self::Contains(needle) => match value {
                Some(DatabaseValue::String(text)) => {
                    text.to_lowercase().contains(&needle.to_lowercase())
                }
                _ => false,
            },
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ViewFilter {
    pub field: Option<Uuid>,
    pub condition: FilterCondition,
}

impl ViewFilter {
    pub fn matches(&self, row: &DatabaseRow) -> bool {
        self.field
            .is_none_or(|field| self.condition.matches(row.value(field)))
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DatabaseView {
    pub database: Option<Uuid>,
    pub sort: Option<DatabaseViewSort>,
    pub kind: DatabaseViewKind,
    pub kanban_field: Option<Uuid>,
    pub scatter_x: Option<Uuid>,
    pub scatter_y: Option<Uuid>,
    pub filters: List<ViewFilter>,
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

    pub fn shows(&self, row: &DatabaseRow) -> bool {
        self.filters.iter().all(|filter| filter.matches(row))
    }

    pub fn add_filter(field: Option<Uuid>, condition: FilterCondition) -> (ObjectId, Edit) {
        let filter = ViewFilter { field, condition };
        let (id, change) = Self::FILTERS.insert(ObjectId::ROOT, Anchor::End, &filter);
        (id, change.into())
    }

    pub fn set_filter_field(filter: ObjectId, field: Option<Uuid>) -> Edit {
        ViewFilter::FIELD.set(filter, &field).into()
    }

    pub fn set_filter_condition(filter: ObjectId, condition: &FilterCondition) -> Edit {
        ViewFilter::CONDITION.set(filter, condition).into()
    }

    pub fn remove_filter(filter: ObjectId) -> Edit {
        Change::remove(filter).into()
    }
}

impl Root for DatabaseView {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x0000_6461_7461_6261_7365_2d76_6965_7701);

    fn references(&self) -> Vec<Uuid> {
        self.database.into_iter().collect()
    }
}

pub type DatabaseViewContent = Document<DatabaseView>;
