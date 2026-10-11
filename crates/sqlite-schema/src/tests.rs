use super::*;

mod a_changed_index_is_rebuilt_as_declared;
mod a_column_that_changed_or_cannot_be_added_is_a_mismatch;
mod a_new_table_index_and_defaulted_column_are_added_in_place;

const FIRST: &str = "
    CREATE TABLE notes (id TEXT PRIMARY KEY, body TEXT NOT NULL);
";
