use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::{DatabaseRow, DatabaseValue};
use crate::database_schema::DatabaseField;
use be_block::Root;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default)]
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
    AnyOf(Vec<DatabaseValue>),
    NoneOf(Vec<DatabaseValue>),
    IsOpen,
    IsClosed,
}

impl FilterCondition {
    pub fn matches(&self, value: Option<&DatabaseValue>, field: Option<&DatabaseField>) -> bool {
        let closed = || match value {
            Some(DatabaseValue::Enum(option)) => field.is_some_and(|field| field.closes(*option)),
            _ => false,
        };
        match self {
            Self::IsSet => value.is_some(),
            Self::IsEmpty => value.is_none(),
            Self::Is(wanted) => holds(value, wanted),
            Self::IsNot(unwanted) => !holds(value, unwanted),
            Self::AnyOf(wanted) => wanted.iter().any(|wanted| holds(value, wanted)),
            Self::NoneOf(unwanted) => !unwanted.iter().any(|unwanted| holds(value, unwanted)),
            Self::IsOpen => !closed(),
            Self::IsClosed => closed(),
            Self::Contains(needle) => match value {
                Some(DatabaseValue::String(text)) => {
                    text.to_lowercase().contains(&needle.to_lowercase())
                }
                _ => false,
            },
        }
    }
}

fn holds(value: Option<&DatabaseValue>, wanted: &DatabaseValue) -> bool {
    match (value, wanted) {
        (Some(DatabaseValue::Links(targets)), DatabaseValue::Relation(target)) => {
            targets.contains(target)
        }
        (value, wanted) => value == Some(wanted),
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ViewFilter {
    pub field: Option<Uuid>,
    pub condition: FilterCondition,
}

impl ViewFilter {
    pub fn matches(&self, row: &DatabaseRow, fields: &[DatabaseField]) -> bool {
        self.field.is_none_or(|field| {
            let schema = fields.iter().find(|known| known.id == field);
            self.condition.matches(row.get(field).as_ref(), schema)
        })
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DatabaseView {
    pub database: Option<Uuid>,
    pub sort: Option<DatabaseViewSort>,
    pub kind: DatabaseViewKind,
    #[model(rename = "kanban_field")]
    pub group_by: Option<Uuid>,
    pub scatter_x: Option<Uuid>,
    pub scatter_y: Option<Uuid>,
    pub filters: List<ViewFilter>,
    pub hidden_fields: Vec<Uuid>,
    pub show_empty_groups: bool,
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

    pub fn set_group_by(field: Option<Uuid>) -> Edit {
        Self::GROUP_BY.set(ObjectId::ROOT, &field).into()
    }

    pub fn set_hidden_fields(fields: Vec<Uuid>) -> Edit {
        Self::HIDDEN_FIELDS.set(ObjectId::ROOT, &fields).into()
    }

    pub fn set_show_empty_groups(shown: bool) -> Edit {
        Self::SHOW_EMPTY_GROUPS.set(ObjectId::ROOT, &shown).into()
    }

    pub fn shows_field(&self, field: Uuid) -> bool {
        !self.hidden_fields.contains(&field)
    }

    pub fn set_scatter_x(field: Option<Uuid>) -> Edit {
        Self::SCATTER_X.set(ObjectId::ROOT, &field).into()
    }

    pub fn set_scatter_y(field: Option<Uuid>) -> Edit {
        Self::SCATTER_Y.set(ObjectId::ROOT, &field).into()
    }

    pub fn shows(&self, row: &DatabaseRow, fields: &[DatabaseField]) -> bool {
        self.filters
            .iter()
            .all(|filter| filter.matches(row, fields))
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

    const FROZEN: bool = true;

    fn references(&self) -> Vec<Uuid> {
        self.database.into_iter().collect()
    }
}

pub type DatabaseViewContent = Document<DatabaseView>;
