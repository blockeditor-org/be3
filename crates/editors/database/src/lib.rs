pub mod app;
pub mod database_schema;
pub mod database_view;

block_editor_beui::beui_plugin!("../manifest.json", {
    database => app::DatabaseApp,
    schema => database_schema::app::DatabaseSchemaApp,
    view => database_view::app::DatabaseViewApp,
});

#[cfg(test)]
mod tests;
