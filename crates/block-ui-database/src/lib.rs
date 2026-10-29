use std::collections::{BTreeMap, BTreeSet, HashMap};

use database_types::{
    database::{DatabaseColor, DatabaseRow, DatabaseValue, RowRef, short_id},
    database_schema::{DatabaseField, DatabaseFieldType, DatabaseNumberScale},
};
use uuid::Uuid;

use block_ui::{
    BlockLabel,
    datetime::{format_datetime_utc, parse_datetime_utc},
};

#[derive(Debug, PartialEq)]
pub struct DatabaseValueChange {
    pub field_id: Uuid,
    pub value: Option<DatabaseValue>,
    pub continuous: bool,
}

#[derive(Clone, Default, PartialEq)]
pub struct ValueLabels {
    pub blocks: HashMap<Uuid, BlockLabel>,
    pub rows: BTreeMap<RowRef, String>,
    pub unreachable: BTreeSet<Uuid>,
}

impl ValueLabels {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn row(&self, link: &RowRef) -> String {
        if self.unreachable.contains(&link.database) {
            return block_ui::Reach::NoAccess
                .label()
                .unwrap_or_default()
                .to_owned();
        }
        self.rows
            .get(link)
            .cloned()
            .unwrap_or_else(|| short_id("", link.row))
    }

    pub fn rows_of(&self, database: Uuid) -> Vec<(RowRef, String)> {
        let mut rows: Vec<(RowRef, String)> = self
            .rows
            .iter()
            .filter(|(link, _)| link.database == database)
            .map(|(link, label)| (*link, label.clone()))
            .collect();
        rows.sort_by(|a, b| {
            a.1.to_lowercase()
                .cmp(&b.1.to_lowercase())
                .then(a.0.cmp(&b.0))
        });
        rows
    }
}

pub fn cell_text(row: &DatabaseRow, field: &DatabaseField, labels: &ValueLabels) -> String {
    match row.get(field.id) {
        Some(value) => database_value_text(&value, field, labels),
        None => String::new(),
    }
}

pub fn database_value_text(
    value: &DatabaseValue,
    field: &DatabaseField,
    labels: &ValueLabels,
) -> String {
    match value {
        DatabaseValue::String(value) => value.clone(),
        DatabaseValue::Number(value) => value.to_string(),
        DatabaseValue::Enum(id) => field
            .enum_options
            .iter()
            .find(|option| option.id == *id)
            .map_or_else(String::new, |option| option.name.clone()),
        DatabaseValue::Block(reference) => block_reference_text(reference, &labels.blocks),
        DatabaseValue::Boolean(value) => value.to_string(),
        DatabaseValue::Color(color) => format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.red, color.green, color.blue, color.alpha
        ),
        DatabaseValue::Datetime(value) => format_datetime_utc(*value),
        DatabaseValue::Relation(link) => labels.row(link),
        DatabaseValue::Links(links) => links
            .iter()
            .map(|link| labels.row(link))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

pub fn block_reference_text(reference: &Uuid, block_labels: &HashMap<Uuid, BlockLabel>) -> String {
    block_labels
        .get(reference)
        .map_or_else(|| reference.to_string(), |label| label.name.clone())
}

pub fn parse_cell_value(value: &str, field: &DatabaseField) -> Option<DatabaseValue> {
    match field.field_type {
        DatabaseFieldType::String => Some(DatabaseValue::String(value.to_owned())),
        DatabaseFieldType::Number => {
            let mut value = value.parse::<f64>().ok()?;
            if !value.is_finite()
                || (field.number_options.scale == DatabaseNumberScale::Logarithmic && value <= 0.0)
            {
                return None;
            }
            if let Some(minimum) = field.number_options.minimum {
                value = value.max(minimum);
            }
            if let Some(maximum) = field.number_options.maximum {
                value = value.min(maximum);
            }
            Some(DatabaseValue::Number(value))
        }
        DatabaseFieldType::Enum => field
            .enum_options
            .iter()
            .find(|option| option.name == value)
            .map(|option| DatabaseValue::Enum(option.id)),
        DatabaseFieldType::Block | DatabaseFieldType::Relation => None,
        DatabaseFieldType::Boolean => value.parse().ok().map(DatabaseValue::Boolean),
        DatabaseFieldType::Color => {
            let value = value.strip_prefix('#')?;
            if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }
            let channel = |start| u8::from_str_radix(&value[start..start + 2], 16).ok();
            Some(DatabaseValue::Color(DatabaseColor {
                red: channel(0)?,
                green: channel(2)?,
                blue: channel(4)?,
                alpha: channel(6)?,
            }))
        }
        DatabaseFieldType::Datetime => parse_datetime_utc(value).map(DatabaseValue::Datetime),
    }
}

pub fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "Text",
        DatabaseFieldType::Number => "Number",
        DatabaseFieldType::Enum => "Choice",
        DatabaseFieldType::Block => "Block",
        DatabaseFieldType::Boolean => "Checkbox",
        DatabaseFieldType::Color => "Color",
        DatabaseFieldType::Datetime => "Date and time",
        DatabaseFieldType::Relation => "Link to rows",
    }
}

#[cfg(test)]
mod tests;
