pub mod app;
pub mod database_schema;
pub mod database_view;

block_editor_plugin::beui_plugin!("../manifest.json", {
    block_editor_plugin::be_block::DatabaseContent => app::DatabaseApp,
    block_editor_plugin::be_block::DatabaseSchemaContent => database_schema::app::DatabaseSchemaApp,
    block_editor_plugin::be_block::DatabaseViewContent => database_view::app::DatabaseViewApp,
});

#[cfg(test)]
mod tests;
