pub mod checkout;
pub mod repository;

block_editor_beui::beui_plugin!("../manifest.json", {
    repository => repository::RepositoryApp,
    checkout => checkout::CheckoutApp,
});

#[cfg(test)]
mod tests;
