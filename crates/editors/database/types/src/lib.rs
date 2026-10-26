pub mod database;
pub mod database_schema;
pub mod database_view;

pub use database::{Database, DatabaseContent};
pub use database_schema::{DatabaseSchema, DatabaseSchemaContent};
pub use database_view::{DatabaseView, DatabaseViewContent};

block_type_guest::be_block_types![
    DatabaseContent { undo },
    DatabaseSchemaContent { undo },
    DatabaseViewContent { undo },
];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
