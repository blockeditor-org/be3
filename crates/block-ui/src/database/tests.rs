use super::*;
use be_block::database_schema::DatabaseEnumOption;
use uuid::Uuid;

fn field(field_type: DatabaseFieldType) -> DatabaseField {
    DatabaseField {
        id: Uuid::new_v4(),
        name: "Field".to_owned(),
        field_type,
        enum_options: Vec::new(),
        number_options: Default::default(),
        block_options: Default::default(),
    }
}
mod color_datetime_and_boolean_text_round_trip;
mod enum_value_formats_as_option_name;
mod invalid_typed_values_do_not_produce_replacements;
mod number_parsing_accepts_valid_and_rejects_invalid_and_empty;
mod string_empty_is_stored;
mod typed_numbers_clamp_to_configured_boundaries;

use be_block::{
    database::{DatabaseColor, DatabaseValue},
    database_schema::{DatabaseNumberOptions, DatabaseNumberScale},
};
