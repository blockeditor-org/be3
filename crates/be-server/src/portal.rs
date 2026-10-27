use crate::{
    ServerError, Services, browser_auth,
    mail::Email,
    store::{AccountSession, MailLink, Profile},
};
use base64::Engine;
use be_portal::{Account, Asset, Page, Portal, Problem, Screen, Session, SessionKind};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(crate) struct Response {
    pub status: u16,
    pub body: Body,
    pub location: Option<String>,
    pub cookie: Option<String>,
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(be_protocol::ErrorCode::InvalidRequest, message.into())
}

pub(crate) use be_portal::encode;

use be_portal::with_code as link;

fn deliver(services: &Services, origin: &str, mail: MailLink) {
    let page = if mail.purpose == "reset" {
        "reset"
    } else {
        "verify"
    };
    services.mail.send(Email {
        recipient: mail.email,
        subject: if mail.purpose == "reset" { "Reset your Block password" } else { "Verify your Block email address" }.into(),
        body: format!("Open {origin}/auth/{page}?token={} . If you did not request this, ignore this message.", encode(&mail.token)),
    });
}

pub(crate) async fn handle(
    services: &Services,
    method: &str,
    path: &str,
    query: &HashMap<String, String>,
    fields: &HashMap<String, String>,
    token: &str,
    origin: &str,
) -> Result<Response, ServerError> {
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
        let confirm_password = || if value("password") == value("confirmation") { Ok(()) } else { Err(invalid("Password confirmation: the passwords do not match.")) };
        match path {
            "/auth/login" => {
                login_allowed()?;
                let (_, credential) = services.store.login_browser(value("email"), value("password")).await?;
                if !token.is_empty() { services.store.logout(token).await?; }
                response.cookie = Some(credential);
                response.location = Some(link("/auth", &code));
            },
            "/auth/register" => {
                login_allowed()?;
                if !services.config.allow_registration { return Err(invalid("This server is not accepting new accounts.")); }
                confirm_password()?;
                let mail = services.store.registration_link(value("email"), value("display_name"), value("password"), &code).await?;
                deliver(services, origin, mail);
                response.location = Some(link("/auth/check-email", &code));
            },
            "/auth/forgot" => {
                login_allowed()?;
                if let Some(mail) = services.store.password_reset_link(value("email"), &code).await? { deliver(services, origin, mail); }
                response.location = Some(link("/auth/reset-requested", &code));
            },
            "/auth/verify" => {
                let purpose = services.store.mail_purpose(value("token")).await?;
                if purpose == "register" {
                    login_allowed()?;
                    if !services.config.allow_registration { return Err(invalid("This server is not accepting new accounts.")); }
                }
                let (registered, saved_code, old_email) = services.store.confirm_mail(value("token"), None).await?;
                if let Some(account) = registered {
                    if !token.is_empty() { services.store.logout(token).await?; }
                    response.cookie = Some(services.store.browser_session(account).await?);
                    response.location = Some(link("/auth", &saved_code));
                } else {
                    if let Some(email) = old_email { services.mail.send(Email { recipient: email, subject: "Your Block email address changed".into(), body: "Your account email address was changed. If this was not you, contact your server operator.".into() }); }
                    response.location = Some("/auth/email-updated".into());
                }
            },
            "/auth/reset" => {
                login_allowed()?;
                confirm_password()?;
                let (_, saved_code, _) = services.store.confirm_mail(value("token"), Some(value("password"))).await?;
                response.cookie = Some(String::new());
                response.location = Some(link("/auth/password-reset", &saved_code));
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
                let mail = services.store.email_change_link(account()?, value("email"), value("current_password")).await?;
                deliver(services, origin, mail);
                response.location = Some("/auth/check-email".into());
            },
            "/auth/settings/password" => {
                confirm_password()?;
                response.cookie = Some(services.store.change_password(account()?, value("current_password"), value("password"), token).await?);
                response.location = Some("/auth/settings?saved=1".into());
            },
            "/auth/sessions/revoke" => {
                services.store.revoke_session(account()?, value("session")).await?;
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
    if profile.is_none() && matches!(display_path.as_str(), "/auth/settings" | "/auth/sessions") {
        response.status = 303;
        response.location = Some(link("/auth/login", &code));
        return Ok(response);
    }
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
        "/auth/register" | "/auth/forgot" | "/auth/reset" if !config.allow_login => {
            Screen::SignInClosed
        }
        "/auth/register" if !config.allow_registration => Screen::RegistrationClosed,
        "/auth/register" => Screen::Register,
        "/auth/forgot" => Screen::Forgot,
        "/auth/reset" => Screen::Reset,
        "/auth/verify" => Screen::Verify,
        "/auth/settings" => Screen::Settings,
        "/auth/sessions" => Screen::Sessions,
        "/auth/approved" => Screen::Approved,
        "/auth/denied" => Screen::Denied,
        "/auth/check-email" => Screen::CheckEmail,
        "/auth/reset-requested" => Screen::ResetRequested,
        "/auth/password-reset" => Screen::PasswordReset,
        "/auth/email-updated" => Screen::EmailUpdated,
        _ => Screen::NotFound,
    };
    let known = matches!(
        path,
        "/auth"
            | "/auth/login"
            | "/auth/register"
            | "/auth/forgot"
            | "/auth/verify"
            | "/auth/reset"
            | "/auth/settings"
            | "/auth/sessions"
            | "/auth/approved"
            | "/auth/denied"
            | "/auth/check-email"
            | "/auth/reset-requested"
            | "/auth/password-reset"
            | "/auth/email-updated"
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
        account: profile.as_ref().map(|profile| Account {
            display_name: profile.display_name.clone(),
            email: profile.email.clone(),
        }),
        code,
        code_problem,
        csrf: services.browser_auth.form(token)?,
        problem: (!error.is_empty()).then(|| Problem::from_message(&error)),
        mail_token: value("token").into(),
        sessions: sessions.into_iter().map(session).collect(),
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
            SessionKind::App
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
