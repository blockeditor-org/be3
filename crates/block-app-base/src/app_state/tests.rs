use super::*;

mod active_account_round_trips;
mod client_id_persists_across_reopen;
mod display_settings_persist_across_reopen;
mod fresh_store_has_no_accounts;
mod identity_secrets_belong_to_their_account_and_leave_with_it;
mod input_settings_persist_across_reopen;
mod plugins_in_use_persist_across_reopen;
mod removing_an_account_clears_active_selection;
mod saved_accounts_and_last_organization_survive_reopen;
mod the_last_organization_belongs_to_its_account_and_leaves_with_it;

fn account(server: ServerLocation, email: &str) -> SavedAccount {
    SavedAccount {
        server,
        id: Uuid::new_v4(),
        email: email.into(),
        name: email.into(),
        token: Uuid::new_v4().to_string(),
        last_organization_id: None,
        authorization: None,
    }
}
