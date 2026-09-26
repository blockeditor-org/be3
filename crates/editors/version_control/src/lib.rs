pub mod checkout;
pub mod repository;

block_editor_beui::beui_plugin!("../manifest.json", {
    block_editor_beui::be_block::RepositoryContent => repository::RepositoryApp,
    block_editor_beui::be_block::CheckoutContent => checkout::CheckoutApp,
});

#[cfg(test)]
mod tests;
