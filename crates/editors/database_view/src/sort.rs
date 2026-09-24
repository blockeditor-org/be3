use std::cmp::Ordering;
use std::collections::HashMap;

use block_client::blocks::database::{DatabaseRow, DatabaseValue};
use block_client::blocks::database_schema::{DatabaseField, DatabaseFieldType};
use block_client::blocks::database_view::{DatabaseViewSort, SortDirection};
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::block_ui::database::block_reference_text;
use uuid::Uuid;

pub type BlockLabels = HashMap<Uuid, BlockLabel>;

pub const ROW_HEADER_WIDTH: f32 = 44.0;
pub const ROW_HEIGHT: f32 = 28.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayRow {
    pub index: usize,
    pub row: DatabaseRow,
}

pub fn next_sort(current: Option<DatabaseViewSort>, field_id: Uuid) -> Option<DatabaseViewSort> {
    match current {
        Some(sort) if sort.field_id == field_id => match sort.direction {
            SortDirection::Ascending => Some(DatabaseViewSort {
                field_id,
                direction: SortDirection::Descending,
            }),
            SortDirection::Descending => None,
        },
        _ => Some(DatabaseViewSort {
            field_id,
            direction: SortDirection::Ascending,
        }),
    }
}

pub fn display_rows(
    rows: &[DatabaseRow],
    selected: Option<usize>,
    sort: Option<DatabaseViewSort>,
    fields: &[DatabaseField],
    labels: &BlockLabels,
) -> Vec<DisplayRow> {
    let mut display: Vec<DisplayRow> = rows
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, row)| DisplayRow { index, row })
        .collect();
    if let Some(sort) = sort
        && let Some(field) = fields.iter().find(|field| field.id == sort.field_id)
    {
        display.sort_by(|a, b| compare_sorted_rows(&a.row, &b.row, sort, field, labels));
    }
    for offset in 0..extra_row_count(rows.len(), selected) {
        display.push(DisplayRow {
            index: rows.len() + offset,
            row: DatabaseRow::default(),
        });
    }
    display
}

pub fn extra_row_count(len: usize, selected: Option<usize>) -> usize {
    match selected {
        Some(selected) if selected >= len => (selected - len + 2).max(1),
        _ => 1,
    }
}

pub fn sort_rows(
    rows: &mut [DatabaseRow],
    sort: DatabaseViewSort,
    fields: &[DatabaseField],
    labels: &BlockLabels,
) {
    let Some(field) = fields.iter().find(|field| field.id == sort.field_id) else {
        return;
    };
    rows.sort_by(|a, b| compare_sorted_rows(a, b, sort, field, labels));
}

fn compare_sorted_rows(
    a: &DatabaseRow,
    b: &DatabaseRow,
    sort: DatabaseViewSort,
    field: &DatabaseField,
    labels: &BlockLabels,
) -> Ordering {
    let ordering = match (a.value(sort.field_id), b.value(sort.field_id)) {
        (Some(a), Some(b)) => compare_database_values(a, b, field, labels),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    match sort.direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
    }
}

pub fn compare_database_values(
    a: &DatabaseValue,
    b: &DatabaseValue,
    field: &DatabaseField,
    labels: &BlockLabels,
) -> Ordering {
    match (a, b) {
        (DatabaseValue::String(a), DatabaseValue::String(b)) => a.cmp(b),
        (DatabaseValue::Number(a), DatabaseValue::Number(b)) => {
            a.partial_cmp(b).unwrap_or(Ordering::Equal)
        }
        (DatabaseValue::Enum(a), DatabaseValue::Enum(b)) => {
            let index = |id: Uuid| field.enum_options.iter().position(|option| option.id == id);
            match (index(*a), index(*b)) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        }
        (DatabaseValue::Boolean(a), DatabaseValue::Boolean(b)) => a.cmp(b),
        (DatabaseValue::Color(a), DatabaseValue::Color(b)) => {
            [a.red, a.green, a.blue, a.alpha].cmp(&[b.red, b.green, b.blue, b.alpha])
        }
        (DatabaseValue::Datetime(a), DatabaseValue::Datetime(b)) => a.cmp(b),
        (DatabaseValue::Block(a), DatabaseValue::Block(b)) => block_reference_text(a, labels)
            .cmp(&block_reference_text(b, labels))
            .then_with(|| a.cmp(b)),
        _ => Ordering::Equal,
    }
}

pub fn column_width(field_type: DatabaseFieldType) -> f32 {
    match field_type {
        DatabaseFieldType::String
        | DatabaseFieldType::Enum
        | DatabaseFieldType::Block
        | DatabaseFieldType::Datetime => 180.0,
        DatabaseFieldType::Number => 120.0,
        DatabaseFieldType::Boolean => 90.0,
        DatabaseFieldType::Color => 140.0,
    }
}

pub fn column_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            return name;
        }
        index = index / 26 - 1;
    }
}
