pub mod checkout;
pub mod repository;

be_sdk_beui::beui_plugin!("../manifest.json", {
    repository => repository::RepositoryRoot,
    checkout => checkout::CheckoutRoot,
});

#[cfg(test)]
mod tests;
