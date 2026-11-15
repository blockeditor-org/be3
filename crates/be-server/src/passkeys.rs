//! Passkeys (WebAuthn): the ceremony half (`store/passkeys.rs` is the accounts half). The
//! relying party is the `--auth-origin` host, so a passkey works only at the origin it was made
//! for. A page that offers a passkey gets its challenge with its form token
//! (`browser_auth::Sessions::form`), and posting that form spends both: the challenge is bound
//! to the page's session, single use and as short-lived as the token, and the post passes the
//! portal's Origin check like every form. The portal's wasm runs the browser's ceremony and
//! posts its answer in a form of its own (be-server-portal-ui's `passkey.rs`).

use be_server_portal::{PasskeyOffer, PasskeyStep};
use uuid::Uuid;
use webauthn_rs::prelude::{
    DiscoverableAuthentication, PasskeyAuthentication, PasskeyRegistration, Url, Webauthn,
    WebauthnBuilder,
};

use crate::ServerError;

fn refused(message: &str) -> ServerError {
    ServerError::Refused(be_block_protocol::ErrorCode::InvalidRequest, message.into())
}

/// The RP ID of the portal at `origin`: its host, unless that is an IP address.
pub(crate) fn rp_id(origin: &str) -> Option<String> {
    Url::parse(origin).ok()?.domain().map(str::to_owned)
}

/// The RP ID passkeys are made for at start: `--auth-origin`'s, or with none `localhost`, the
/// only host with an RP ID the portal serves without one.
pub(crate) fn configured_rp_id(config: &crate::ServerConfig) -> Option<String> {
    rp_id(config.auth_origin.as_deref().unwrap_or("http://localhost"))
}

/// The relying party for the portal at `origin`, its host as the RP ID; none when the host is
/// an IP address, which WebAuthn has no RP ID for.
pub(crate) fn relying_party(origin: &str) -> Option<Webauthn> {
    let host = rp_id(origin)?;
    let origin = Url::parse(origin).ok()?;
    WebauthnBuilder::new(&host, &origin)
        .ok()?
        .rp_name("be3")
        .timeout(crate::browser_auth::LIFETIME)
        .build()
        .ok()
}

/// A ceremony a page began, waiting for the browser's answer.
pub(crate) enum Ceremony {
    /// Signing in with any passkey the browser discovers.
    SignIn(DiscoverableAuthentication),
    /// Proving the signed-in account is the browser's with one of its own passkeys.
    Proof {
        account: Uuid,
        state: PasskeyAuthentication,
    },
    /// Adding a passkey to the signed-in account.
    Add {
        account: Uuid,
        state: PasskeyRegistration,
    },
}

/// Begins a ceremony for `step`, and answers the options the browser's call takes with it.
/// `passkeys` are the signed-in account's, for a proof or to leave out when adding.
pub(crate) fn begin(
    relying_party: &Webauthn,
    step: PasskeyStep,
    account: Option<(Uuid, &str)>,
    passkeys: &[webauthn_rs::prelude::Passkey],
) -> Result<(PasskeyOffer, Ceremony), ServerError> {
    let failed = |_| refused("Passkeys are unavailable right now. Sign in another way.");
    let (options, ceremony) = match (step, account) {
        (PasskeyStep::SignIn, _) => {
            let (mut options, state) = relying_party
                .start_discoverable_authentication()
                .map_err(failed)?;
            // Asked for by a button, not offered in an email field's autofill.
            options.mediation = None;
            (
                serde_json::to_string(&options.public_key),
                Ceremony::SignIn(state),
            )
        }
        (PasskeyStep::Proof, Some((account, _))) => {
            let (options, state) = relying_party
                .start_passkey_authentication(passkeys)
                .map_err(failed)?;
            (
                serde_json::to_string(&options.public_key),
                Ceremony::Proof { account, state },
            )
        }
        (PasskeyStep::Add, Some((account, name))) => {
            let made = passkeys.iter().map(|key| key.cred_id().clone()).collect();
            let (options, state) = relying_party
                .start_passkey_registration(account, name, name, Some(made))
                .map_err(failed)?;
            // Discoverable, so it signs in before any email is typed.
            let options = serde_json::to_value(&options.public_key).map(|mut options| {
                options["authenticatorSelection"]["residentKey"] = "required".into();
                options["authenticatorSelection"]["requireResidentKey"] = true.into();
                options.to_string()
            });
            (options, Ceremony::Add { account, state })
        }
        (_, None) => return Err(refused("Sign in to continue.")),
    };
    let options =
        options.map_err(|_| refused("Passkeys are unavailable right now. Sign in another way."))?;
    Ok((PasskeyOffer { step, options }, ceremony))
}

/// The browser's answer to a ceremony, as `PublicKeyCredential.toJSON()` gives it.
pub(crate) fn answer<T: serde::de::DeserializeOwned>(credential: &str) -> Result<T, ServerError> {
    serde_json::from_str(credential)
        .map_err(|_| refused("Your browser’s passkey answer could not be read. Try again."))
}

/// A refusal for a passkey answer the relying party did not accept.
pub(crate) fn not_accepted(_: webauthn_rs::prelude::WebauthnError) -> ServerError {
    refused(crate::store::passkeys::DID_NOT_WORK)
}

/// How the settings page names a passkey: when it was added (UTC), from its identity's
/// `created`, and the start of its credential id, from its `subject`, so two added the same
/// minute still differ.
pub(crate) fn label(created: i64, subject: &str) -> String {
    let credential = subject.rsplit(' ').next().unwrap_or_default();
    let short: String = credential.chars().take(6).collect();
    format!("Passkey added {} ({short})", utc_minute(created))
}

/// A time in seconds since the epoch as the portal shows it, to the minute in UTC:
/// `2026-11-15 14:02 UTC`.
pub(crate) fn utc_minute(seconds: i64) -> String {
    let days = seconds.div_euclid(86400);
    let of_day = seconds.rem_euclid(86400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let day_of_era = z - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 { shifted + 3 } else { shifted - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    let (hour, minute) = (of_day / 3600, of_day % 3600 / 60);
    format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}
