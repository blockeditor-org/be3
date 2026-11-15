//! Signing in and registering by an emailed code, taken from the store rather than a mailbox.

use be_server::ServerStore;
use be_server::store::codes::{self, CodePurpose, CodeRequest, EnteredCode};

/// Asks for a code for `email` and enters it, as a browser at `address` would. Registering
/// takes the display name and the invite code; the other purposes ignore them.
pub(crate) async fn enter(
    store: &ServerStore,
    config: be_server::ServerConfig,
    purpose: CodePurpose,
    email: &str,
    name: &str,
    invite: &str,
    address: &str,
) -> Result<EnteredCode, String> {
    let attempt = codes::new_attempt();
    let mailed = store
        .request_code(CodeRequest {
            purpose,
            email,
            account: None,
            display_name: name,
            invite,
            device_code: "",
            attempt: &attempt,
            address,
        })
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("{email} has no account to send a code to"))?;
    store
        .enter_code(&attempt, &mailed.code, "", address, |_, _| true, config)
        .await
        .map_err(|error| error.to_string())
}
