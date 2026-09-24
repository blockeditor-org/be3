use std::collections::HashMap;

use block_client::blocks::{
    database::{DatabaseColor, DatabaseRow, DatabaseValue},
    database_schema::{DatabaseField, DatabaseFieldType, DatabaseNumberScale},
};
use uuid::Uuid;

use crate::{
    BlockLabel,
    datetime::{format_datetime_utc, parse_datetime_utc},
};

#[derive(Debug, PartialEq)]
pub struct DatabaseValueChange {
    pub field_id: Uuid,
    pub value: Option<DatabaseValue>,
    pub continuous: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseBlockPickRequest {
    pub field_id: Uuid,
    pub block_type: Option<Uuid>,
}

pub fn cell_text(
    row: &DatabaseRow,
    field: &DatabaseField,
    block_labels: &HashMap<Uuid, BlockLabel>,
) -> String {
    match row.value(field.id) {
        Some(value) => database_value_text(value, field, block_labels),
        None => String::new(),
    }
}

pub fn database_value_text(
    value: &DatabaseValue,
    field: &DatabaseField,
    block_labels: &HashMap<Uuid, BlockLabel>,
) -> String {
    match value {
        DatabaseValue::String(value) => value.clone(),
        DatabaseValue::Number(value) => value.to_string(),
        DatabaseValue::Enum(id) => field
            .enum_options
            .iter()
            .find(|option| option.id == *id)
            .map_or_else(String::new, |option| option.name.clone()),
        DatabaseValue::Block(reference) => block_reference_text(reference, block_labels),
        DatabaseValue::Boolean(value) => value.to_string(),
        DatabaseValue::Color(color) => format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.red, color.green, color.blue, color.alpha
        ),
        DatabaseValue::Datetime(value) => format_datetime_utc(*value),
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
        DatabaseFieldType::Block => None,
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
        DatabaseFieldType::Enum => "Enum",
        DatabaseFieldType::Block => "Block",
        DatabaseFieldType::Boolean => "Boolean",
        DatabaseFieldType::Color => "Color",
        DatabaseFieldType::Datetime => "Datetime",
    }
}

#[cfg(test)]
mod tests;
