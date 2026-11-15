use crate::{
    ServerError, Services, browser_auth,
    mail::Email,
    store::{
        AccountSession, MailLink, Profile, ServerStore,
        codes::{self, CodePurpose, CodeRequest, MailedCode},
    },
};
use base64::Engine;
use be_server_portal::{
    Account, Asset, IdentityRow, Page, Portal, Problem, Screen, Session, SessionKind,
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

/// Tells the account's email that a way to sign in was removed.
fn identity_notice(services: &Services, email: &str, removed: &str) {
    services.mail.send(Email {
        recipient: email.into(),
        subject: "A way to sign in to your be3 account was removed".into(),
        body: format!("{removed} can no longer sign in to your account. If this was not you, sign in and check your account, or contact your server operator."),
    });
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
    let code = value("code").trim().to_ascii_lowercase();
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
        if method != "POST" { return Ok(String::new()); }
        services.browser_auth.consume_form(value("csrf"), token)?;
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
                let attempt = attempt_for(&mut response);
                let mailed = services.store.request_code(CodeRequest { purpose: CodePurpose::Proof, email: &profile.email, account: Some(profile.account), display_name: "", invite: "", device_code: "", attempt: &attempt, address }).await?;
                if let Some(mailed) = mailed { deliver_code(services, origin, mailed); }
                response.location = Some("/auth/enter".into());
            },
            "/auth/enter" => {
                let entered = services.store.enter_code(attempt, value("email_code"), token, address, |purpose, config| match purpose {
                    CodePurpose::Register => config.allow_login && config.allow_registration,
                    CodePurpose::Proof => true,
                    _ => config.allow_login,
                }, services.config.clone()).await?;
                response.attempt = Some(String::new());
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
                let removed = services.store.remove_identity(account, token, value("identity")).await?;
                if let Some(profile) = profile.as_ref() { identity_notice(services, &profile.email, &removed.subject); }
                response.location = Some("/auth/settings?saved=1".into());
            },
            "/auth/verify" => {
                let old_email = services.store.confirm_mail(value("token")).await?;
                services.mail.send(Email { recipient: old_email, subject: "Your be3 email address changed".into(), body: "Your account email address was changed. If this was not you, contact your server operator.".into() });
                response.location = Some("/auth/email-updated".into());
            },
            "/auth/code" => {
                browser_auth::code_status(services, &code)?;
                response.location = Some(link("/auth", &code));
            },
            "/auth/approve" => {
                browser_auth::decide(services, &code, Some(account()?), token).await?;
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
    if path == "/auth/proof" {
        display_path = "/auth/settings".into();
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
    if !code.is_empty()
        && let Err(error) = browser_auth::code_status(services, &code)
    {
        code_problem = error.to_string();
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
        "/auth/settings" => Screen::Settings,
        "/auth/sessions" => Screen::Sessions,
        "/auth/approved" => Screen::Approved,
        "/auth/denied" => Screen::Denied,
        "/auth/check-email" => Screen::CheckEmail,
        "/auth/email-updated" => Screen::EmailUpdated,
        "/auth/recover" if !config.allow_login => Screen::SignInClosed,
        "/auth/recover" => Screen::Recover,
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
            | "/auth/settings"
            | "/auth/sessions"
            | "/auth/approved"
            | "/auth/denied"
            | "/auth/check-email"
            | "/auth/email-updated"
            | "/auth/enter"
            | "/auth/recover"
    );
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
    let page = Page {
        screen,
        email: field("email", |profile| &profile.email),
        display_name: field("display_name", |profile| &profile.display_name),
        invite: fields.get("invite").cloned().unwrap_or_default(),
        account: profile.as_ref().map(|profile| Account {
            display_name: profile.display_name.clone(),
            email: profile.email.clone(),
        }),
        code,
        code_problem,
        csrf: services.browser_auth.form(token, client),
        problem: (!error.is_empty()).then(|| Problem::from_message(&error)),
        mail_token: value("token").into(),
        sessions: sessions.into_iter().map(session).collect(),
        identities: {
            let last = identities.len() <= 1;
            identities
                .into_iter()
                .map(|identity| IdentityRow {
                    label: identity.subject,
                    kind: identity.kind,
                    id: identity.id,
                    removable: !last,
                })
                .collect()
        },
        fresh,
        code_email,
        email_code: linked_code.filter(|_| linked_here).unwrap_or_default(),
        saved: query.contains_key("saved"),
        allow_registration: config.allow_registration,
        assets: String::new(),
    };
    response.body = show(services, page);
    Ok(response)
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
