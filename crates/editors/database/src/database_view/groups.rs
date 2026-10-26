use database_types::database::{DatabaseRow, DatabaseValue, RowRef};
use database_types::database_schema::{DatabaseEnumOption, DatabaseField, DatabaseFieldType};
use uuid::Uuid;

use crate::database_view::sort::BlockLabels;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GroupKey {
    All,
    Empty,
    Option(Uuid),
    Row(RowRef),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub key: GroupKey,
    pub label: String,
    pub option: Option<DatabaseEnumOption>,
    pub closed: bool,
    pub rows: Vec<usize>,
}

impl Group {
    pub fn value(&self) -> Option<DatabaseValue> {
        match self.key {
            GroupKey::Option(option) => Some(DatabaseValue::Enum(option)),
            GroupKey::Row(row) => Some(DatabaseValue::Relation(row)),
            GroupKey::All | GroupKey::Empty => None,
        }
    }
}

pub fn groupable(field: &DatabaseField) -> bool {
    match field.field_type {
        DatabaseFieldType::Enum => true,
        DatabaseFieldType::Relation => !field.relation_options.multiple,
        _ => false,
    }
}

pub fn group_rows(
    rows: &[DatabaseRow],
    shown: &[usize],
    field: Option<&DatabaseField>,
    labels: &BlockLabels,
    show_empty: bool,
) -> Vec<Group> {
    let Some(field) = field.filter(|field| groupable(field)) else {
        return vec![Group {
            key: GroupKey::All,
            label: String::new(),
            option: None,
            closed: false,
            rows: shown.to_vec(),
        }];
    };
    let key_of = |index: usize| match rows.get(index).and_then(|row| row.get(field.id)) {
        Some(DatabaseValue::Enum(option)) if field.option(option).is_some() => {
            GroupKey::Option(option)
        }
        Some(DatabaseValue::Relation(link)) => GroupKey::Row(link),
        Some(DatabaseValue::Links(links)) if links.len() == 1 => GroupKey::Row(links[0]),
        _ => GroupKey::Empty,
    };
    let mut groups: Vec<Group> = match field.field_type {
        DatabaseFieldType::Enum => field
            .enum_options
            .iter()
            .map(|option| Group {
                key: GroupKey::Option(option.id),
                label: option.name.clone(),
                option: Some(option.clone()),
                closed: option.closed,
                rows: Vec::new(),
            })
            .collect(),
        _ => {
            let mut linked: Vec<RowRef> = shown
                .iter()
                .filter_map(|index| match key_of(*index) {
                    GroupKey::Row(link) => Some(link),
                    _ => None,
                })
                .collect();
            if show_empty && let Some(database) = field.relation_options.database {
                linked.extend(labels.rows_of(database).into_iter().map(|(link, _)| link));
            }
            linked.sort_by_key(|link| (labels.row(link).to_lowercase(), *link));
            linked.dedup();
            linked
                .into_iter()
                .map(|link| Group {
                    key: GroupKey::Row(link),
                    label: labels.row(&link),
                    option: None,
                    closed: false,
                    rows: Vec::new(),
                })
                .collect()
        }
    };
    groups.push(Group {
        key: GroupKey::Empty,
        label: format!("No {}", field.name.to_lowercase()),
        option: None,
        closed: false,
        rows: Vec::new(),
    });
    for index in shown {
        let key = key_of(*index);
        if let Some(group) = groups.iter_mut().find(|group| group.key == key) {
            group.rows.push(*index);
        }
    }
    groups.retain(|group| !group.rows.is_empty() || (show_empty && group.key != GroupKey::Empty));
    groups
}

pub fn matches_search(row: &DatabaseRow, title: Option<Uuid>, code: &str, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let titled = title.and_then(|field| match row.value(field) {
        Some(DatabaseValue::String(text)) => Some(text.to_lowercase()),
        _ => None,
    });
    titled.is_some_and(|text| text.contains(&query)) || code.to_lowercase().contains(&query)
}
