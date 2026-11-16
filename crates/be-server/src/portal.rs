use crate::{
    ServerError, Services, browser_auth,
    mail::Email,
    passkeys,
    store::{
        AccountSession, MailLink, Profile, ServerStore,
        account_notices::ChangeKind,
        codes::{self, CodePurpose, CodeRequest, MailedCode},
        providers::{self, Intent, Outcome},
    },
};
use base64::Engine;
use be_server_portal::{
    Account, Asset, IdentityChange, IdentityRow, Page, PasskeyStep, Portal, Problem, ProviderLink,
    Screen, Session, SessionKind,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(crate) struct Response {
    pub status: u16,
    pub body: Body,
    pub location: Option<String>,
    pub cookie: Option<String>,
    /// The browser's code attempt (`be3-attempt`): a new one, or empty to end it.
    pub attempt: Option<String>,
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(be_block_protocol::ErrorCode::InvalidRequest, message.into())
}

pub(crate) use be_server_portal::encode;

use be_server_portal::with_code as link;

/// Mails a code, with a link that fills it in in the browser that asked for it.
fn deliver_code(services: &Services, origin: &str, mail: MailedCode) {
    let (subject, doing) = match mail.purpose {
        CodePurpose::SignIn => ("Your be3 sign-in code", "sign in"),
        CodePurpose::Recover => ("Your be3 recovery code", "get back into your account"),
        CodePurpose::Register => ("Your be3 registration code", "finish creating your account"),
        CodePurpose::Proof => ("Your be3 confirmation code", "confirm it is you"),
        CodePurpose::Link => (
            "Your be3 code to add a way to sign in",
            "sign in and add the provider you just used",
        ),
    };
    services.mail.send(Email {
        recipient: mail.email,
        subject: subject.into(),
        body: format!(
            "Your code is {code}. Enter it in the browser where you asked for it to {doing}, or open {origin}/auth/enter?attempt={attempt}&email_code={code} there. It works for {minutes} minutes. If you did not ask for it, ignore this message: nobody can use it without that browser.",
            code = mail.code,
            attempt = mail.attempt,
            minutes = codes::CODE_LIFETIME / 60,
        ),
    });
}

/// Tells the account its ways to sign in changed: on its devices and in the settings page's
/// history (`store/account_notices.rs`), and by mail to `email` when it is not empty. While mail
/// goes only to the server's log, the history and the notices are how the account hears of it.
/// `recorded` says the history row was already written where the change was made
/// (`claim_identity_add`).
async fn tell_identity(
    services: &Services,
    account: uuid::Uuid,
    email: &str,
    kind: ChangeKind,
    label: &str,
    recorded: bool,
) {
    let (subject, text) = match kind {
        ChangeKind::Added => (
            "A way to sign in to your be3 account was added",
            format!(
                "{label} can now sign in to your account. If this was not you, sign in and remove it, or contact your server operator."
            ),
        ),
        ChangeKind::Removed => (
            "A way to sign in to your be3 account was removed",
            format!(
                "{label} can no longer sign in to your account. If this was not you, sign in and check your account, or contact your server operator."
            ),
        ),
        ChangeKind::Email => (
            "Your be3 email address changed",
            format!(
                "Your account's email changed to {label}. If this was not you, sign in and check your account, or contact your server operator."
            ),
        ),
    };
    tell(services, account, email, subject, text, kind, vec![label.to_owned()], recorded).await;
}

/// Finishes taking an account back once its recovery signed in (`codes::recover_account`):
/// deletes every agent it made, then tells it in one mail and one notice what went, every way
/// to sign in but its email and those agents (plans/crypto_audit.md, 9).
async fn finish_recovery(
    services: &Services,
    account: uuid::Uuid,
    email: &str,
    removed: &[codes::Identity],
) -> Result<(), ServerError> {
    let agents = services.store.delete_agents_created_by(account).await?;
    if removed.is_empty() && agents.is_empty() {
        return Ok(());
    }
    let labels: Vec<String> = removed.iter().map(|identity| identity_label(&services.config, identity)).collect();
    let mut went = labels.clone();
    went.extend(agents.iter().map(|name| format!("the agent {name}")));
    let text = format!(
        "Recovering your account removed every way to sign in but your email, and every agent you made, since someone else may have added them: {}. Add back the ones you trust.",
        went.join(", ")
    );
    tell(services, account, email, "Recovering your be3 account removed ways in", text, ChangeKind::Removed, labels, false).await;
    Ok(())
}

/// Mails `email` when it is not empty, and records the change for the settings page's history
/// and the account's devices, whose connections are told.
#[allow(clippy::too_many_arguments)]
async fn tell(
    services: &Services,
    account: uuid::Uuid,
    email: &str,
    subject: &str,
    text: String,
    kind: ChangeKind,
    labels: Vec<String>,
    recorded: bool,
) {
    if !email.is_empty() {
        services.mail.send(Email {
            recipient: email.into(),
            subject: subject.into(),
            body: text.clone(),
        });
    }
    if services
        .store
        .identity_changed(account, kind, labels, text, recorded)
        .await
        .is_ok()
    {
        // From no connection: every connection of the account hears it.
        services
            .hub
            .send_account(account, 0, None, &crate::watch::Push::AccountNoticesChanged)
            .await;
    }
}

/// Tells an account's old email that the account's email changed.
fn email_moved_notice(services: &Services, old: String) {
    services.mail.send(Email {
        recipient: old,
        subject: "Your be3 email address changed".into(),
        body: "Your account email address was changed. If this was not you, contact your server operator.".into(),
    });
}

/// How a notice names a provider identity by its subject.
pub(crate) fn provider_identity_label(config: &crate::ServerConfig, subject: &str) -> String {
    identity_label(
        config,
        &codes::Identity {
            id: String::new(),
            kind: "oidc".into(),
            subject: subject.into(),
            email: String::new(),
            created: 0,
        },
    )
}

/// Mails the link that verifies a new email for the account.
fn deliver(services: &Services, origin: &str, mail: MailLink) {
    services.mail.send(Email {
        recipient: mail.email,
        subject: "Verify your be3 email address".into(),
        body: format!("Open {origin}/auth/verify?token={} . If you did not request this, ignore this message.", encode(&mail.token)),
    });
}

pub(crate) async fn handle(
    services: &Services,
    method: &str,
    path: &str,
    query: &HashMap<String, String>,
    fields: &HashMap<String, String>,
    token: &str,
    attempt: &str,
    origin: &str,
    client: std::net::IpAddr,
) -> Result<Response, ServerError> {
    let address = client.to_string();
    let address = address.as_str();
    let value = |name: &str| {
        fields
            .get(name)
            .or_else(|| query.get(name))
            .map(String::as_str)
            .unwrap_or_default()
    };
    // A provider's sign-in, begun or sent back: the provider's id, and which.
    let oidc = path
        .strip_prefix("/auth/oidc/")
        .and_then(|rest| rest.split_once('/'))
        .filter(|(_, step)| matches!(*step, "start" | "callback"));
    // The provider's callback carries its own `code`, which is no browser sign-in's.
    let code = match oidc {
        Some((_, "callback")) => String::new(),
        _ => value("code").trim().to_ascii_lowercase(),
    };
    let profile = services.store.browser_profile(token).await.ok();
    let mut response = Response {
        status: 200,
        body: Body::default(),
        location: None,
        cookie: None,
        attempt: None,
    };
    if method != "GET" && method != "POST" {
        response.status = 405;
        response.body = message(
            services,
            "Method not allowed",
            "Use the page’s forms to continue.",
        );
        return Ok(response);
    }
    let result: Result<String, ServerError> = async {
        if let Some((provider, step)) = oidc.filter(|_| method == "GET") {
            return provider_step(services, &mut response, provider, step, query, profile.as_ref(), token, attempt, origin, address).await;
        }
        if method != "POST" { return Ok(String::new()); }
        let ceremony = services.browser_auth.consume_form(value("csrf"), token)?;
        let account = || profile.as_ref().map(|profile| profile.account).ok_or_else(|| invalid("Sign in to continue."));
        let login_allowed = || if services.config.allow_login { Ok(()) } else { Err(invalid("This server is not accepting sign-ins.")) };
        let registration_allowed = || if services.config.allow_registration { Ok(()) } else { Err(invalid("This server is not accepting new accounts.")) };
        // The browser's attempt: the one it holds, or a new one it is given with the response.
        let attempt_for = |response: &mut Response| match attempt.is_empty() {
            true => {
                let fresh = codes::new_attempt();
                response.attempt = Some(fresh.clone());
                fresh
            }
            false => attempt.to_owned(),
        };
        match path {
            "/auth/code/send" => {
                login_allowed()?;
                let purpose = match value("purpose") { "recover" => CodePurpose::Recover, _ => CodePurpose::SignIn };
                let attempt = attempt_for(&mut response);
                let mailed = services.store.request_code(CodeRequest { purpose, email: value("email"), account: None, display_name: "", invite: "", device_code: &code, attempt: &attempt, address }).await?;
                if let Some(mailed) = mailed { deliver_code(services, origin, mailed); }
                response.location = Some(link("/auth/enter", &code));
            },
            "/auth/register" => {
                login_allowed()?;
                registration_allowed()?;
                let attempt = attempt_for(&mut response);
                let mailed = services.store.request_code(CodeRequest { purpose: CodePurpose::Register, email: value("email"), account: None, display_name: value("display_name"), invite: value("invite"), device_code: &code, attempt: &attempt, address }).await?;
                if let Some(mailed) = mailed { deliver_code(services, origin, mailed); }
                response.location = Some(link("/auth/enter", &code));
            },
            "/auth/proof" => {
                let profile = profile.as_ref().ok_or_else(|| invalid("Sign in to continue."))?;
                if profile.email.is_empty() {
                    return Err(invalid("Your account has no email: confirm it is you with a passkey or a provider you sign in with."));
                }
                let attempt = attempt_for(&mut response);
                let mailed = services.store.request_code(CodeRequest { purpose: CodePurpose::Proof, email: &profile.email, account: Some(profile.account), display_name: "", invite: "", device_code: "", attempt: &attempt, address }).await?;
                if let Some(mailed) = mailed { deliver_code(services, origin, mailed); }
                response.location = Some("/auth/enter".into());
            },
            "/auth/oidc/register" => {
                login_allowed()?;
                registration_allowed()?;
                let (session, device_code) = services.store.register_by_provider(attempt, value("invite"), value("display_name")).await?;
                if !token.is_empty() { services.store.logout(token).await?; }
                response.cookie = Some(session);
                response.attempt = Some(String::new());
                response.location = Some(link("/auth", &device_code));
            },
            "/auth/enter" => {
                let entered = services.store.enter_code(attempt, value("email_code"), token, address, |purpose, config| match purpose {
                    CodePurpose::Register => config.allow_login && config.allow_registration,
                    CodePurpose::Proof => true,
                    _ => config.allow_login,
                }, services.config.clone()).await?;
                response.attempt = Some(String::new());
                if let Some(linked) = &entered.linked { tell_identity(services, entered.account, &entered.email, ChangeKind::Added, &provider_identity_label(&services.config, linked), false).await; }
                if entered.purpose == CodePurpose::Recover { finish_recovery(services, entered.account, &entered.email, &entered.removed).await?; }
                match entered.session {
                    Some(session) => {
                        if !token.is_empty() { services.store.logout(token).await?; }
                        response.cookie = Some(session);
                        response.location = Some(link("/auth", &entered.device_code));
                    }
                    None => response.location = Some("/auth/settings?proved=1".into()),
                }
            },
            "/auth/settings/identity/remove" => {
                let account = account()?;
                let removed = services.store.remove_identity(account, token, value("identity"), issuers(&services.config), passkeys::rp_id(origin)).await?;
                if let Some(profile) = profile.as_ref() { tell_identity(services, profile.account, &profile.email, ChangeKind::Removed, &identity_label(&services.config, &removed), false).await; }
                response.location = Some("/auth/settings?saved=1".into());
            },
            "/auth/passkey/sign-in" => {
                login_allowed()?;
                let Some(passkeys::Ceremony::SignIn(state)) = ceremony else { return Err(invalid("Open the sign-in page and try again.")) };
                let relying_party = passkeys::relying_party(origin).ok_or_else(|| invalid("This server has no passkeys."))?;
                let credential: webauthn_rs::prelude::PublicKeyCredential = passkeys::answer(value("credential"))?;
                let (handle, credential_id) = relying_party.identify_discoverable_authentication(&credential).map_err(passkeys::not_accepted)?;
                let rp_id = passkeys::rp_id(origin).unwrap_or_default();
                let found = services.store.passkey(&rp_id, credential_id).await?;
                // The user handle a passkey was made with is its account's id.
                if handle != found.account { return Err(passkeys::not_accepted(webauthn_rs::prelude::WebauthnError::InvalidUserUniqueId)); }
                let result = relying_party.finish_discoverable_authentication_v2(&credential, state, &found.passkey).map_err(passkeys::not_accepted)?;
                let session = services.store.passkey_signed_in(found, result, address).await?;
                if !token.is_empty() { services.store.logout(token).await?; }
                response.cookie = Some(session);
                response.location = Some(link("/auth", &code));
            },
            "/auth/passkey/proof" => {
                let account = account()?;
                let Some(passkeys::Ceremony::Proof { account: began, state }) = ceremony else { return Err(invalid("Open your account settings and try again.")) };
                if began != account { return Err(invalid("Open your account settings and try again.")); }
                let relying_party = passkeys::relying_party(origin).ok_or_else(|| invalid("This server has no passkeys."))?;
                let credential: webauthn_rs::prelude::PublicKeyCredential = passkeys::answer(value("credential"))?;
                let result = relying_party.finish_passkey_authentication(&credential, &state).map_err(passkeys::not_accepted)?;
                services.store.passkey_proved(&passkeys::rp_id(origin).unwrap_or_default(), account, token, result).await?;
                response.location = Some("/auth/settings?proved=1".into());
            },
            "/auth/passkey/add" => {
                let account = account()?;
                let Some(passkeys::Ceremony::Add { account: began, state }) = ceremony else { return Err(invalid("Open your account settings and try again.")) };
                if began != account { return Err(invalid("Open your account settings and try again.")); }
                services.store.require_fresh(token).await?;
                services.store.identity_add_room(account).await?;
                let relying_party = passkeys::relying_party(origin).ok_or_else(|| invalid("This server has no passkeys."))?;
                let credential: webauthn_rs::prelude::RegisterPublicKeyCredential = passkeys::answer(value("credential"))?;
                let passkey = relying_party.finish_passkey_registration(&credential, &state).map_err(passkeys::not_accepted)?;
                let added = services.store.add_passkey(&passkeys::rp_id(origin).unwrap_or_default(), account, token, passkey).await?;
                if let Some(profile) = profile.as_ref() { tell_identity(services, profile.account, &profile.email, ChangeKind::Added, &identity_label(&services.config, &added), true).await; }
                response.location = Some("/auth/settings?saved=1".into());
            },
            "/auth/link" => {
                let used = services.store.use_login_link(value("token"), address).await?;
                finish_recovery(services, used.account, &used.email, &used.removed).await?;
                if !token.is_empty() { services.store.logout(token).await?; }
                response.cookie = Some(used.session);
                response.location = Some("/auth".into());
            },
            "/auth/verify" => {
                let (account, old_email, new_email) = services.store.confirm_mail(value("token")).await?;
                if !old_email.is_empty() { email_moved_notice(services, old_email); }
                // The old address was mailed above; the new one verified it, and needs no mail.
                tell_identity(services, account, "", ChangeKind::Email, &new_email, false).await;
                response.location = Some("/auth/email-updated".into());
            },
            "/auth/code" => {
                browser_auth::code_status(services, &code)?;
                response.location = Some(link("/auth", &code));
            },
            "/auth/approve" => {
                let account = account()?;
                // A cookie alone, however old, mints no token (plans/crypto_audit.md, 8).
                services.store.require_fresh(token).await?;
                browser_auth::decide(services, &code, Some(account), token).await?;
                response.location = Some("/auth/approved".into());
            },
            "/auth/deny" => {
                browser_auth::decide(services, &code, None, token).await?;
                response.location = Some("/auth/denied".into());
            },
            "/auth/logout" => {
                services.store.logout(token).await?;
                response.cookie = Some(String::new());
                response.location = Some(link("/auth/login", &code));
            },
            "/auth/settings/name" => {
                services.store.update_display_name(account()?, value("display_name")).await?;
                response.location = Some("/auth/settings?saved=1".into());
            },
            "/auth/settings/email" => {
                let mail = services.store.email_change_link(account()?, value("email"), token, address).await?;
                deliver(services, origin, mail);
                response.location = Some("/auth/check-email".into());
            },
            "/auth/sessions/revoke" => {
                services.store.revoke_session(account()?, value("session"), token).await?;
                if services.store.browser_profile(token).await.is_err() { response.cookie = Some(String::new()); }
                response.location = Some("/auth/sessions".into());
            },
            "/auth/sessions/others" => {
                services.store.revoke_other_sessions(account()?, token).await?;
                response.location = Some("/auth/sessions".into());
            },
            _ => return Err(invalid("Unknown form. Open the account portal and try again.")),
        }
        Ok(String::new())
    }.await;
    let error = match result {
        Ok(message) => message,
        Err(error) => {
            response.status = 400;
            response.location = None;
            error.to_string()
        }
    };
    // Keep the attempt's cookie for as long as its code, counted from now, while it is used.
    if response.attempt.is_none() && !attempt.is_empty() {
        response.attempt = Some(attempt.to_owned());
    }
    if response.location.is_some() {
        response.status = 303;
        return Ok(response);
    }
    let mut display_path = path.to_string();
    if path.starts_with("/auth/settings/") {
        display_path = "/auth/settings".into();
    }
    if path.starts_with("/auth/sessions/") {
        display_path = "/auth/sessions".into();
    }
    if matches!(path, "/auth/code" | "/auth/approve" | "/auth/deny") {
        display_path = "/auth".into();
    }
    if path == "/auth/code/send" {
        display_path = match value("purpose") {
            "recover" => "/auth/recover".into(),
            _ => "/auth/login".into(),
        };
    }
    if path == "/auth/proof" || path.starts_with("/auth/passkey/") {
        display_path = match profile.is_some() {
            true => "/auth/settings".into(),
            false => "/auth/login".into(),
        };
    }
    // A provider's sign-in that failed shows where it began.
    if oidc.is_some() {
        display_path = match profile.is_some() {
            true => "/auth/settings".into(),
            false => "/auth/login".into(),
        };
    }
    if profile.is_none() && matches!(display_path.as_str(), "/auth/settings" | "/auth/sessions") {
        response.status = 303;
        response.location = Some(link("/auth/login", &code));
        return Ok(response);
    }
    let identities = match (display_path.as_str(), profile.as_ref()) {
        ("/auth/settings", Some(profile)) => services.store.identities(profile.account).await?,
        _ => Vec::new(),
    };
    let fresh = match display_path.as_str() {
        "/auth/settings" | "/auth/sessions" if profile.is_some() => {
            services.store.is_fresh(token).await?
        }
        // An approval needs the same proof (plans/crypto_audit.md, 8).
        "/auth" if profile.is_some() && !code.is_empty() => services.store.is_fresh(token).await?,
        _ => false,
    };
    // A code from the mail's link: filled in where its attempt began, only shown anywhere else.
    let linked_code = query
        .get("email_code")
        .map(|code| codes::normalize_code(code))
        .filter(|code| codes::is_well_formed(code));
    let linked_here = query
        .get("attempt")
        .is_some_and(|public| ServerStore::is_attempt(attempt, public));
    let code_email = match display_path.as_str() {
        "/auth/enter" => services
            .store
            .attempt_email(attempt)
            .await
            .unwrap_or_default(),
        _ => String::new(),
    };
    let sessions = if display_path == "/auth/sessions" {
        services
            .store
            .account_sessions(profile.as_ref().unwrap().account, token)
            .await?
    } else {
        Vec::new()
    };
    let mut code_problem = String::new();
    let mut requester = String::new();
    let mut requester_elsewhere = false;
    if !code.is_empty() {
        match browser_auth::code_status(services, &code) {
            Ok((asking, age)) => {
                let when = session_age(crate::store::portal::now() - age.as_secs() as i64);
                requester_elsewhere = crate::address::address_key(asking) != crate::address::address_key(client);
                requester = match requester_elsewhere {
                    false => format!("Asked {when} from {asking}, the same network as this browser."),
                    true => format!("Asked {when} from {asking}, not this browser's network. Deny unless you just started signing in on another device."),
                };
            }
            Err(error) => code_problem = error.to_string(),
        }
    }
    let config = &services.config;
    let signed_in = profile.is_some();
    let screen = match display_path.as_str() {
        "/auth" | "/auth/login" if !config.allow_login && !signed_in => Screen::SignInClosed,
        "/auth"
            if signed_in && !code.is_empty() && code_problem.is_empty() && config.allow_login =>
        {
            Screen::Approve
        }
        "/auth" if signed_in => Screen::Account,
        "/auth" | "/auth/login" => Screen::SignIn,
        "/auth/register" if !config.allow_login => Screen::SignInClosed,
        "/auth/register" if !config.allow_registration => Screen::RegistrationClosed,
        "/auth/register" => Screen::Register,
        "/auth/verify" => Screen::Verify,
        "/auth/link" => Screen::LoginLink,
        "/auth/settings" => Screen::Settings,
        "/auth/sessions" => Screen::Sessions,
        "/auth/approved" => Screen::Approved,
        "/auth/denied" => Screen::Denied,
        "/auth/check-email" => Screen::CheckEmail,
        "/auth/email-updated" => Screen::EmailUpdated,
        "/auth/recover" if !config.allow_login => Screen::SignInClosed,
        "/auth/recover" => Screen::Recover,
        "/auth/oidc/register" if !config.allow_login => Screen::SignInClosed,
        "/auth/oidc/register" if !config.allow_registration => Screen::RegistrationClosed,
        "/auth/oidc/register" => Screen::ProviderRegister,
        "/auth/enter" => match &linked_code {
            Some(code) if !linked_here => Screen::CodeElsewhere { code: code.clone() },
            _ => Screen::EnterCode,
        },
        _ => Screen::NotFound,
    };
    let known = matches!(
        path,
        "/auth"
            | "/auth/login"
            | "/auth/register"
            | "/auth/verify"
            | "/auth/link"
            | "/auth/settings"
            | "/auth/sessions"
            | "/auth/approved"
            | "/auth/denied"
            | "/auth/check-email"
            | "/auth/email-updated"
            | "/auth/enter"
            | "/auth/recover"
            | "/auth/oidc/register"
    ) || oidc.is_some();
    if !known && method == "GET" {
        response.status = 404;
    }
    let field = |name: &str, own: fn(&Profile) -> &String| {
        fields
            .get(name)
            .cloned()
            .or_else(|| profile.as_ref().map(|profile| own(profile).clone()))
            .unwrap_or_default()
    };
    // The passkey ceremony the screen offers, begun with the page's form token.
    let relying_party = passkeys::relying_party(origin);
    let rp_id = passkeys::rp_id(origin);
    let step = match (&screen, profile.as_ref()) {
        (Screen::SignIn, _) => Some(PasskeyStep::SignIn),
        (Screen::Settings, Some(_)) if fresh => Some(PasskeyStep::Add),
        (Screen::Settings | Screen::Sessions, Some(_)) if !fresh => Some(PasskeyStep::Proof),
        _ => None,
    };
    let account = profile
        .as_ref()
        .filter(|_| step.is_some_and(|step| step != PasskeyStep::SignIn))
        .map(|profile| {
            let name = match profile.email.is_empty() {
                true => profile.display_name.clone(),
                false => profile.email.clone(),
            };
            (profile.account, name)
        });
    let (passkey, ceremony) = match (relying_party.as_ref(), step) {
        (Some(relying_party), Some(step)) => {
            let passkeys = match account.as_ref() {
                Some((account, _)) => {
                    let rp_id = passkeys::rp_id(origin).unwrap_or_default();
                    services.store.passkeys(&rp_id, *account).await?
                }
                None => Vec::new(),
            };
            // A proof needs a passkey of the account's own.
            match step == PasskeyStep::Proof && passkeys.is_empty() {
                true => (None, None),
                false => {
                    let account = account
                        .as_ref()
                        .map(|(account, name)| (*account, name.as_str()));
                    let (offer, ceremony) =
                        passkeys::begin(relying_party, step, account, &passkeys)?;
                    (Some(offer), Some(ceremony))
                }
            }
        }
        _ => (None, None),
    };
    let page = Page {
        screen,
        email: field("email", |profile| &profile.email),
        display_name: match display_path.as_str() {
            "/auth/oidc/register" if !fields.contains_key("display_name") => services
                .store
                .pending_provider(attempt)
                .await
                .map(|(_, name)| name)
                .unwrap_or_default(),
            _ => field("display_name", |profile| &profile.display_name),
        },
        invite: fields.get("invite").cloned().unwrap_or_default(),
        account: profile.as_ref().map(|profile| Account {
            display_name: profile.display_name.clone(),
            email: profile.email.clone(),
        }),
        code: code.clone(),
        code_problem,
        requester,
        requester_elsewhere,
        csrf: services.browser_auth.form(token, client, ceremony),
        problem: (!error.is_empty()).then(|| Problem::from_message(&error)),
        mail_token: value("token").into(),
        sessions: sessions.into_iter().map(session).collect(),
        identities: {
            // As `remove_identity` judges it.
            let issuers = issuers(config);
            let works = |identity: &codes::Identity| {
                providers::signs_in(
                    &identity.kind,
                    &identity.subject,
                    &issuers,
                    rp_id.as_deref(),
                )
            };
            identities
                .iter()
                .map(|identity| IdentityRow {
                    label: identity_label(config, identity),
                    kind: identity.kind.clone(),
                    id: identity.id.clone(),
                    removable: identities
                        .iter()
                        .any(|other| other.id != identity.id && works(other)),
                })
                .collect()
        },
        providers: match display_path.as_str() {
            "/auth" | "/auth/login" | "/auth/register" if !signed_in => config
                .providers
                .iter()
                .map(|provider| ProviderLink {
                    name: provider.name.clone(),
                    href: link(&format!("/auth/oidc/{}/start", provider.id), &code),
                })
                .collect(),
            "/auth/settings" => config
                .providers
                .iter()
                .filter(|provider| !linked(&identities, provider))
                .map(|provider| ProviderLink {
                    name: provider.name.clone(),
                    href: format!(
                        "/auth/oidc/{}/start?intent=link&check={}",
                        provider.id,
                        providers::start_check(token)
                    ),
                })
                .collect(),
            _ => Vec::new(),
        },
        proof_providers: config
            .providers
            .iter()
            .filter(|provider| linked(&identities, provider))
            .map(|provider| ProviderLink {
                name: provider.name.clone(),
                href: format!(
                    "/auth/oidc/{}/start?intent=proof&check={}",
                    provider.id,
                    providers::start_check(token)
                ),
            })
            .collect(),
        provider: match display_path.as_str() {
            "/auth/oidc/register" => match services.store.pending_provider(attempt).await {
                Some((provider, _)) => provider_name(config, &provider),
                None => String::new(),
            },
            "/auth/enter" => query
                .get("linking")
                .map(|provider| provider_name(config, provider))
                .unwrap_or_default(),
            _ => String::new(),
        },
        identity_changes: match (display_path.as_str(), profile.as_ref()) {
            ("/auth/settings", Some(profile)) => services
                .store
                .identity_changes(profile.account)
                .await?
                .into_iter()
                .map(|change| IdentityChange {
                    text: match change.kind {
                        ChangeKind::Added => format!("Added {}", change.label),
                        ChangeKind::Removed => format!("Removed {}", change.label),
                        ChangeKind::Email => format!("Email changed to {}", change.label),
                    },
                    when: passkeys::utc_minute(change.created),
                })
                .collect(),
            _ => Vec::new(),
        },
        fresh,
        passkey,
        code_email,
        email_code: linked_code.filter(|_| linked_here).unwrap_or_default(),
        saved: query.contains_key("saved"),
        allow_registration: config.allow_registration,
        assets: String::new(),
    };
    response.body = show(services, page);
    Ok(response)
}

/// The issuers of the providers the server has.
fn issuers(config: &crate::ServerConfig) -> Vec<String> {
    config
        .providers
        .iter()
        .map(|provider| provider.issuer.clone())
        .collect()
}

/// The configured provider whose id is `id`, by its name; nothing if there is none.
fn provider_name(config: &crate::ServerConfig, id: &str) -> String {
    config
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .map(|provider| provider.name.clone())
        .unwrap_or_default()
}

/// Whether one of the account's identities is the provider's.
fn linked(identities: &[codes::Identity], provider: &crate::oidc::Provider) -> bool {
    identities.iter().any(|identity| {
        identity.kind == "oidc"
            && identity
                .subject
                .starts_with(&format!("{} ", provider.issuer))
    })
}

/// How the settings page names an identity: its email, or its provider's name and the email
/// the provider gave.
fn identity_label(config: &crate::ServerConfig, identity: &codes::Identity) -> String {
    match identity.kind.as_str() {
        "oidc" => {}
        "passkey" => return passkeys::label(identity.created, &identity.subject),
        _ => return identity.subject.clone(),
    }
    let provider = config
        .providers
        .iter()
        .find(|provider| {
            identity
                .subject
                .starts_with(&format!("{} ", provider.issuer))
        })
        .map_or_else(
            || {
                let issuer = identity.subject.split(' ').next().unwrap_or_default();
                format!("A provider this server no longer has ({issuer})")
            },
            |provider| provider.name.clone(),
        );
    match identity.email.is_empty() {
        true => provider,
        false => format!("{provider} ({})", identity.email),
    }
}

/// Begins a sign-in with a provider, or takes the browser back from it.
#[allow(clippy::too_many_arguments)]
async fn provider_step(
    services: &Services,
    response: &mut Response,
    provider: &str,
    step: &str,
    query: &HashMap<String, String>,
    profile: Option<&Profile>,
    token: &str,
    attempt: &str,
    origin: &str,
    address: &str,
) -> Result<String, ServerError> {
    let configured = services
        .config
        .providers
        .iter()
        .find(|configured| configured.id == provider)
        .ok_or_else(|| invalid("This server has no such provider."))?;
    let redirect = format!("{origin}{}", crate::oidc::callback_path(provider));
    let value = |name: &str| query.get(name).map(String::as_str).unwrap_or_default();
    match step {
        "start" => {
            let intent = match value("intent") {
                "" => Intent::SignIn,
                "link" => Intent::Link,
                "proof" => Intent::Proof,
                _ => return Err(invalid("Open the account portal and try again.")),
            };
            let account = match intent {
                Intent::SignIn if !services.config.allow_login => {
                    return Err(invalid("This server is not accepting sign-ins."));
                }
                Intent::SignIn => None,
                _ => Some(
                    profile
                        .ok_or_else(|| invalid("Sign in to continue."))?
                        .account,
                ),
            };
            // A link or a proof begins only from the settings page's own link, which carries a
            // check only this browser's session makes, so another site can't begin one.
            if intent != Intent::SignIn && !providers::is_start_check(token, value("check")) {
                return Err(invalid("Open your account settings and try again."));
            }
            if let (Intent::Link, Some(account)) = (intent, account) {
                services.store.require_fresh(token).await?;
                services.store.identity_add_room(account).await?;
            }
            // Before the provider is called, so strangers can't make the server call it more.
            services.store.provider_room(address).await?;
            // A fresh attempt binds the provider's answer to this browser, and to this sign-in.
            let attempt = codes::new_attempt();
            response.attempt = Some(attempt.clone());
            let started = services
                .discovery
                .start(configured, &redirect, intent != Intent::SignIn)
                .await?;
            services
                .store
                .begin_provider(
                    provider,
                    &started.state,
                    &started.verifier,
                    &started.nonce,
                    &attempt,
                    address,
                    &value("code").trim().to_ascii_lowercase(),
                    intent,
                    account,
                )
                .await?;
            response.location = Some(started.url);
        }
        _ => {
            if !value("error").is_empty() {
                return Err(invalid(&format!(
                    "{} did not sign you in. Try again, or sign in another way.",
                    configured.name
                )));
            }
            let begun = services
                .store
                .take_provider_state(provider, value("state"), attempt)
                .await?;
            let verified = services
                .discovery
                .finish(
                    configured,
                    &redirect,
                    value("code"),
                    &begun.verifier,
                    &begun.nonce,
                    begun.intent != Intent::SignIn,
                )
                .await?;
            let outcome = services
                .store
                .provider_verified(
                    configured,
                    verified,
                    begun,
                    token,
                    attempt,
                    address,
                    &services.config,
                )
                .await?;
            response.location = Some(match outcome {
                Outcome::SignedIn {
                    session,
                    device_code,
                    moved_from,
                } => {
                    if let Some(old) = moved_from {
                        email_moved_notice(services, old);
                        if let Ok(profile) = services.store.browser_profile(&session).await {
                            tell_identity(
                                services,
                                profile.account,
                                "",
                                ChangeKind::Email,
                                &profile.email,
                                false,
                            )
                            .await;
                        }
                    }
                    if !token.is_empty() {
                        services.store.logout(token).await?;
                    }
                    response.cookie = Some(session);
                    link("/auth", &device_code)
                }
                Outcome::Linked => {
                    if let Ok(profile) = services.store.browser_profile(token).await {
                        let added = provider_identity_label(
                            &services.config,
                            &format!("{} ", configured.issuer),
                        );
                        tell_identity(
                            services,
                            profile.account,
                            &profile.email,
                            ChangeKind::Added,
                            &added,
                            true,
                        )
                        .await;
                    }
                    "/auth/settings?saved=1".into()
                }
                Outcome::Proved => "/auth/settings?proved=1".into(),
                Outcome::LinkByCode(mailed) => {
                    deliver_code(services, origin, mailed);
                    format!("/auth/enter?linking={provider}")
                }
                Outcome::NeedsInvite => "/auth/oidc/register".into(),
            });
        }
    }
    Ok(String::new())
}

fn session(session: AccountSession) -> Session {
    Session {
        id: session.id,
        kind: if session.kind == "browser" {
            SessionKind::Browser
        } else {
            SessionKind::Host
        },
        age: session_age(session.created),
        current: session.current,
    }
}

pub(crate) fn message(services: &Services, title: &str, text: &str) -> Body {
    show(
        services,
        Page {
            screen: Screen::Message {
                title: title.into(),
                text: text.into(),
            },
            ..Page::default()
        },
    )
}

fn show(services: &Services, mut page: Page) -> Body {
    match &services.portal {
        Some(portal) => {
            page.assets = portal.base.clone();
            Body::Html((portal.portal.render)(&page))
        }
        None => Body::Json(page.to_json()),
    }
}

fn session_age(created: i64) -> String {
    let seconds = (crate::store::portal::now() - created).max(0);
    for (unit, name) in [(86400, "day"), (3600, "hour"), (60, "minute")] {
        let count = seconds / unit;
        if count > 0 {
            return format!("{count} {name}{} ago", if count == 1 { "" } else { "s" });
        }
    }
    "just now".into()
}

pub(crate) struct Served {
    pub portal: Portal,
    pub base: String,
    pub policy: String,
}

impl Served {
    pub(crate) fn new(portal: Portal) -> Self {
        let mut hash = Sha256::new();
        for asset in portal.assets {
            hash.update(asset.name.as_bytes());
            hash.update([0]);
            hash.update(asset.bytes);
        }
        let version: String = hash.finalize()[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let base = format!("/auth/assets/{version}/");
        let import_map = Sha256::digest((portal.import_map)(&base).as_bytes());
        let policy = format!(
            "default-src 'none'; script-src 'self' 'wasm-unsafe-eval' 'sha256-{}'; worker-src 'self'; connect-src 'self'; font-src 'self'; img-src 'self' data:; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
            base64::engine::general_purpose::STANDARD.encode(import_map)
        );
        Self {
            portal,
            base,
            policy,
        }
    }

    pub(crate) fn asset(&self, path: &str) -> Option<&Asset> {
        let name = path.strip_prefix(&self.base)?;
        self.portal.assets.iter().find(|asset| asset.name == name)
    }
}

pub(crate) enum Body {
    Html(String),
    Json(String),
}

impl Default for Body {
    fn default() -> Self {
        Body::Html(String::new())
    }
}
