pub mod database_schema;
pub mod database_view;
pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    database => root::DatabaseRoot,
    schema => database_schema::root::DatabaseSchemaRoot,
    view => database_view::root::DatabaseViewRoot,
});

#[cfg(test)]
mod tests;
