//! Signing in with an OpenID Connect provider the operator configured (`--oidc-providers`):
//! the network half. This module reads the provider's discovery document, builds the
//! authorization URL with PKCE, `state` and `nonce`, and exchanges the code for an ID token it
//! checks in full (signature against the provider's keys, `iss`, `aud` and `azp`, expiry,
//! nonce). What a checked identity then does to accounts is `store/providers.rs`'s.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use openidconnect::core::{CoreClient, CoreProviderMetadata, CoreResponseType};
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClaimsVerificationError, ClientId, ClientSecret,
    CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    SignatureVerificationError, TokenResponse,
};
use serde::Deserialize;

use crate::ServerError;

/// Google's issuer, which its preset uses.
pub const GOOGLE_ISSUER: &str = "https://accounts.google.com";
/// How long a provider's discovery document and keys are kept before they are read again.
const DISCOVERY_LIFETIME: Duration = Duration::from_secs(3600);
/// How long a call to a provider may take.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a provider whose discovery failed is left alone before it is asked again.
const FAILED_DISCOVERY_HOLD: Duration = Duration::from_secs(30);
/// How often a token signed by a key the server doesn't hold may make it read the keys again.
const KEY_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// How long before the callback a link's or a proof's sign-in at the provider may have been:
/// the begun sign-in's lifetime, and a minute for the clocks.
const REAUTHENTICATED_WITHIN: i64 = crate::store::providers::PROVIDER_SIGN_IN_LIFETIME + 60;

/// A provider the operator configured.
#[derive(Clone)]
pub struct Provider {
    /// Names it in the portal's paths (`/auth/oidc/{id}/...`): lowercase letters, digits and
    /// dashes.
    pub id: String,
    /// What the portal's buttons call it.
    pub name: String,
    /// Its issuer: the tenant's own, never a multi-tenant one.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    /// Whether its `email_verified` is trusted: only then does its email count for anything.
    pub authoritative_email: bool,
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Provider")
            .field("id", &self.id)
            .field("issuer", &self.issuer)
            .field("authoritative_email", &self.authoritative_email)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    preset: Option<String>,
    name: Option<String>,
    issuer: Option<String>,
    client_id: String,
    client_secret: String,
    authoritative_email: Option<bool>,
}

/// Reads the providers file: a JSON list of `{id, name, issuer, client_id, client_secret,
/// authoritative_email}`, where `"preset": "google"` stands for Google's name and issuer and
/// marks it authoritative for email.
pub fn parse_providers(json: &str) -> Result<Vec<Provider>, String> {
    let entries: Vec<Entry> =
        serde_json::from_str(json).map_err(|error| format!("the providers file: {error}"))?;
    let mut providers: Vec<Provider> = Vec::new();
    for entry in entries {
        let id = entry.id;
        let provider = match entry.preset.as_deref() {
            Some("google") => {
                if entry.issuer.is_some() || entry.authoritative_email.is_some() {
                    return Err(format!(
                        "provider {id}: the google preset sets its own issuer and email trust"
                    ));
                }
                Provider {
                    name: entry.name.unwrap_or_else(|| "Google".into()),
                    issuer: GOOGLE_ISSUER.into(),
                    authoritative_email: true,
                    id,
                    client_id: entry.client_id,
                    client_secret: entry.client_secret,
                }
            }
            Some(other) => return Err(format!("provider {id}: no preset is called {other}")),
            None => Provider {
                name: entry.name.ok_or(format!("provider {id} needs a name"))?,
                issuer: entry
                    .issuer
                    .ok_or(format!("provider {id} needs an issuer"))?,
                authoritative_email: entry.authoritative_email.unwrap_or(false),
                id,
                client_id: entry.client_id,
                client_secret: entry.client_secret,
            },
        };
        validate(&provider)?;
        if providers
            .iter()
            .any(|other| other.id == provider.id || other.issuer == provider.issuer)
        {
            return Err(format!(
                "provider {}: another provider has its id or issuer",
                provider.id
            ));
        }
        providers.push(provider);
    }
    Ok(providers)
}

fn validate(provider: &Provider) -> Result<(), String> {
    let id = &provider.id;
    if id.is_empty()
        || id.len() > 32
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!(
            "provider {id:?}: an id is up to 32 lowercase letters, digits and dashes"
        ));
    }
    if provider.name.trim().is_empty() || provider.client_id.is_empty() {
        return Err(format!("provider {id} needs a name and a client id"));
    }
    let issuer = url::Url::parse(&provider.issuer)
        .map_err(|error| format!("provider {id}: its issuer: {error}"))?;
    let loopback = matches!(
        issuer.host(),
        Some(url::Host::Ipv4(address)) if address.is_loopback()
    ) || matches!(issuer.host(), Some(url::Host::Ipv6(address)) if address.is_loopback())
        || issuer.host_str() == Some("localhost");
    if issuer.scheme() != "https" && !(issuer.scheme() == "http" && loopback) {
        return Err(format!(
            "provider {id}: its issuer must be https (http only on this machine, for tests)"
        ));
    }
    // Microsoft's multi-tenant issuers answer for every tenant: each signs its own `sub`s, so
    // one tenant's administrator could sign in as another's people.
    let multi_tenant = issuer.host_str() == Some("login.microsoftonline.com")
        && issuer
            .path_segments()
            .and_then(|mut segments| segments.next())
            .is_some_and(|tenant| {
                ["common", "organizations", "consumers"]
                    .iter()
                    .any(|shared| tenant.eq_ignore_ascii_case(shared))
            });
    if multi_tenant
        || issuer.query().is_some()
        || issuer.fragment().is_some()
        || provider.issuer.contains('{')
    {
        return Err(format!(
            "provider {id}: its issuer is one tenant's, with no query or template"
        ));
    }
    Ok(())
}

/// The path, below the portal's origin, a provider sends the browser back to.
pub fn callback_path(provider: &str) -> String {
    format!("/auth/oidc/{provider}/callback")
}

/// The providers' discovery documents, read when first needed and kept for a while. One read
/// per provider runs at a time, and a failed one holds off the next for a while, so sign-ins
/// begun by strangers can't make the server call a provider over and over.
#[derive(Default)]
pub(crate) struct Discovery {
    documents: Mutex<HashMap<String, Read>>,
    reading: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

enum Read {
    Held(Instant, Arc<CoreProviderMetadata>),
    Failed(Instant),
}

/// The secrets of one sign-in begun with a provider, kept until its callback.
pub(crate) struct Started {
    /// Where to send the browser.
    pub url: String,
    pub state: String,
    pub verifier: String,
    pub nonce: String,
}

/// What a provider's checked ID token says.
pub(crate) struct Verified {
    pub subject: String,
    /// Its email, only when the provider is authoritative for email and says it verified it.
    pub email: Option<String>,
    pub name: String,
}

fn agent() -> ureq::Agent {
    // No redirects: a provider's endpoints answer directly, and following one could reach
    // somewhere the server should not.
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(PROVIDER_TIMEOUT)
        .build()
}

fn failed(provider: &Provider, what: &str, error: impl std::fmt::Display) -> ServerError {
    tracing::warn!(provider = provider.id, "{what}: {error}");
    could_not(provider)
}

fn could_not(provider: &Provider) -> ServerError {
    ServerError::Refused(
        be_block_protocol::ErrorCode::InvalidRequest,
        format!(
            "{} could not sign you in. Try again, or sign in another way.",
            provider.name
        ),
    )
}

/// Whether a token's signature failed in a way the provider's newer keys might fix.
fn unknown_key(error: &ClaimsVerificationError) -> bool {
    matches!(
        error,
        ClaimsVerificationError::SignatureVerification(
            SignatureVerificationError::NoMatchingKey | SignatureVerificationError::CryptoError(_)
        )
    )
}

impl Discovery {
    /// The provider's discovery document. `refresh` reads it again, for keys the held one
    /// lacks, unless it was read within `KEY_REFRESH_INTERVAL`.
    async fn document(
        &self,
        provider: &Provider,
        refresh: bool,
    ) -> Result<Arc<CoreProviderMetadata>, ServerError> {
        let held = || -> Option<Result<Arc<CoreProviderMetadata>, ServerError>> {
            match self.documents.lock().unwrap().get(&provider.id)? {
                Read::Held(read, document) => {
                    let kept = match refresh {
                        true => KEY_REFRESH_INTERVAL,
                        false => DISCOVERY_LIFETIME,
                    };
                    (read.elapsed() < kept).then(|| Ok(document.clone()))
                }
                Read::Failed(at) => {
                    (at.elapsed() < FAILED_DISCOVERY_HOLD).then(|| Err(could_not(provider)))
                }
            }
        };
        if let Some(answer) = held() {
            return answer;
        }
        let reading = self
            .reading
            .lock()
            .unwrap()
            .entry(provider.id.clone())
            .or_default()
            .clone();
        let _reading = reading.lock().await;
        // Another sign-in may have read it while this one waited.
        if let Some(answer) = held() {
            return answer;
        }
        let read = async {
            let issuer = IssuerUrl::new(provider.issuer.clone())
                .map_err(|error| failed(provider, "its issuer", error))?;
            tokio::task::spawn_blocking(move || {
                CoreProviderMetadata::discover(&issuer, &agent()).map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| failed(provider, "discovery", error))?
            .map_err(|error| failed(provider, "discovery", error))
        }
        .await;
        let (kept, answer) = match read {
            Ok(document) => {
                let document = Arc::new(document);
                (Read::Held(Instant::now(), document.clone()), Ok(document))
            }
            Err(error) => (Read::Failed(Instant::now()), Err(error)),
        };
        self.documents
            .lock()
            .unwrap()
            .insert(provider.id.clone(), kept);
        answer
    }

    /// Begins a sign-in: the URL to send the browser to, and the secrets its callback needs.
    /// `reauthenticate` asks the provider to sign the person in again, whatever session it
    /// holds, for a link or a proof.
    pub(crate) async fn start(
        &self,
        provider: &Provider,
        redirect: &str,
        reauthenticate: bool,
    ) -> Result<Started, ServerError> {
        let document = self.document(provider, false).await?;
        let client = CoreClient::from_provider_metadata(
            (*document).clone(),
            ClientId::new(provider.client_id.clone()),
            Some(ClientSecret::new(provider.client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect.into())
                .map_err(|error| failed(provider, "redirect", error))?,
        );
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut request = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".into()))
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge);
        if reauthenticate {
            request = request.set_max_age(Duration::ZERO);
        }
        let (url, state, nonce) = request.url();
        Ok(Started {
            url: url.to_string(),
            state: state.secret().clone(),
            verifier: verifier.secret().clone(),
            nonce: nonce.secret().clone(),
        })
    }

    /// Exchanges the code the provider sent back for an ID token, and checks it.
    /// `reauthenticated` requires the token to say the person signed in at the provider just
    /// now, as `start`'s `reauthenticate` asked.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn finish(
        &self,
        provider: &Provider,
        redirect: &str,
        code: &str,
        verifier: &str,
        nonce: &str,
        reauthenticated: bool,
    ) -> Result<Verified, ServerError> {
        let document = self.document(provider, false).await?;
        let client = |document: &CoreProviderMetadata| {
            CoreClient::from_provider_metadata(
                document.clone(),
                ClientId::new(provider.client_id.clone()),
                Some(ClientSecret::new(provider.client_secret.clone())),
            )
            .set_redirect_uri(RedirectUrl::new(redirect.into()).expect("checked when it began"))
        };
        let exchanging = client(&document);
        let (code, verifier) = (code.to_owned(), verifier.to_owned());
        let response = tokio::task::spawn_blocking(move || {
            exchanging
                .exchange_code(AuthorizationCode::new(code))
                .map_err(|error| error.to_string())?
                .set_pkce_verifier(PkceCodeVerifier::new(verifier))
                .request(&agent())
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| failed(provider, "token exchange", error))?
        .map_err(|error| failed(provider, "token exchange", error))?;
        let token = response
            .id_token()
            .ok_or_else(|| failed(provider, "token exchange", "no ID token"))?;
        let nonce = Nonce::new(nonce.into());
        let claims = match token.claims(&client(&document).id_token_verifier(), &nonce) {
            Ok(claims) => claims.clone(),
            // The provider may have rotated its keys since the document was read.
            Err(error) if unknown_key(&error) => {
                let refreshed = self.document(provider, true).await?;
                if Arc::ptr_eq(&refreshed, &document) {
                    return Err(failed(provider, "the ID token", error));
                }
                token
                    .claims(&client(&refreshed).id_token_verifier(), &nonce)
                    .map_err(|error| failed(provider, "the ID token", error))?
                    .clone()
            }
            Err(error) => return Err(failed(provider, "the ID token", error)),
        };
        if reauthenticated {
            let signed_in = claims.auth_time().map(|time| time.timestamp());
            let since = crate::store::portal::now() - REAUTHENTICATED_WITHIN;
            if !signed_in.is_some_and(|time| time >= since) {
                return Err(failed(
                    provider,
                    "the ID token",
                    "it shows no sign-in just now, which a link or a proof needs",
                ));
            }
        }
        let email = match (provider.authoritative_email, claims.email_verified()) {
            (true, Some(true)) => claims
                .email()
                .and_then(|email| crate::store::normalize_email(email).ok()),
            _ => None,
        };
        let name = claims
            .name()
            .and_then(|name| name.get(None))
            .map(|name| name.to_string())
            .unwrap_or_default();
        Ok(Verified {
            subject: claims.subject().to_string(),
            email,
            name,
        })
    }
}
