use crate::{
    ServerError, Services, browser_auth,
    mail::Email,
    store::{AccountSession, MailLink, Profile},
};
use beui_html::{
    Children, ForEach, Node, Stored, Stylesheet, attr, component, elements as html, render, store,
    view,
};
use std::collections::HashMap;

pub(crate) struct Response {
    pub status: u16,
    pub html: String,
    pub location: Option<String>,
    pub cookie: Option<String>,
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(be_protocol::ErrorCode::InvalidRequest, message.into())
}

pub(crate) fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

fn link(path: &str, code: &str) -> String {
    format!("{path}?code={}", encode(code))
}

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
        html: String::new(),
        location: None,
        cookie: None,
    };
    if method != "GET" && method != "POST" {
        response.status = 405;
        response.html = message("Method not allowed", "Use the page’s forms to continue.");
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
    let csrf = services.browser_auth.form(token)?;
    let mut code_error = String::new();
    if !code.is_empty()
        && let Err(error) = browser_auth::code_status(services, &code)
    {
        code_error = error.to_string();
    }
    let data = Page {
        path: display_path,
        code,
        error,
        csrf,
        profile,
        sessions,
        fields: fields.clone(),
        mail_token: value("token").into(),
        allow_login: services.config.allow_login,
        allow_registration: services.config.allow_registration,
        code_error,
        saved: query.contains_key("saved"),
    };
    if !matches!(
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
    ) && method == "GET"
    {
        response.status = 404;
    }
    response.html = render(|| {
        view! {
            <PageView data={data} />
        }
    });
    Ok(response)
}

struct Page {
    path: String,
    code: String,
    error: String,
    csrf: String,
    profile: Option<Profile>,
    sessions: Vec<AccountSession>,
    fields: HashMap<String, String>,
    mail_token: String,
    allow_login: bool,
    allow_registration: bool,
    code_error: String,
    saved: bool,
}

#[component]
fn Static(condition: bool, children: Children<Node>) -> Children<Node> {
    if condition {
        children
    } else {
        Children::default()
    }
}

#[component]
fn Field(
    name: String,
    label: String,
    #[prop(default = String::new())] value: String,
    #[prop(default = "text".into())] kind: String,
    #[prop(default = String::new())] autocomplete: String,
    #[prop(default = String::new())] error: String,
) -> Children<Node> {
    let has_error = !error.is_empty();
    let aria_invalid = has_error.to_string();
    view! {
        <html::Label>
            {label}
            <html::Input
                name={name}
                value={value}
                r#type={kind}
                autocomplete={autocomplete}
                @attr={("aria-invalid", aria_invalid)}
            />
        </html::Label>
        <Static condition={has_error}>
            <html::Span role="alert">{error}</html::Span>
        </Static>
    }
}

#[component]
fn Form(action: String, csrf: String, code: String, children: Children<Node>) -> Node {
    view! {
        <html::Form method="post" action={action} novalidate=true>
            <html::Input r#type="hidden" name="csrf" value={csrf} />
            <html::Input r#type="hidden" name="code" value={code} />
            {children}
        </html::Form>
    }
}

const STYLES: &str = "
body {
    font-size: 1rem;
    font-family: system-ui, sans-serif;
    margin: 3rem auto;
    padding: 0 1rem;
    max-width: 28rem;
    color: #202124;
    background: #fafafa;
}
label {
    display: block;
    margin: 1rem 0;
}
input {
    display: block;
    box-sizing: border-box;
    width: 100%;
    padding: .7rem;
    font: inherit;
}
button {
    padding: .7rem;
    margin: .4rem .4rem .4rem 0;
    font: inherit;
    cursor: pointer;
}
[role=alert] {
    color: #a00;
}
";

#[component]
fn Shell(children: Children<Node>) -> Node {
    view! {
        <html::Html lang="en">
            <html::Head>
                <html::Meta charset="utf-8" />
                <html::Meta name="viewport" content="width=device-width, initial-scale=1" />
                <html::Title>"Block account"</html::Title>
                <Stylesheet css={STYLES.to_string()} />
            </html::Head>
            <html::Body>
                <html::Header>
                    <html::A href="/auth">"Block account"</html::A>
                </html::Header>
                <html::Main>{children}</html::Main>
            </html::Body>
        </html::Html>
    }
}

pub(crate) fn message(title: &str, message: &str) -> String {
    render(|| {
        view! {
            <Shell>
                <html::H1>{title}</html::H1>
                <html::P>{message}</html::P>
                <html::A href="/auth">"Back to account"</html::A>
            </Shell>
        }
    })
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

fn owned(value: Stored<String>) -> String {
    value.value().as_ref().clone()
}

#[component]
fn PageView(data: Page) -> Node {
    let Page {
        path,
        code,
        error,
        csrf,
        profile,
        sessions,
        fields,
        mail_token,
        allow_login,
        allow_registration,
        code_error,
        saved,
    } = data;
    let signed_in = profile.is_some();
    let account_name = profile
        .as_ref()
        .map(|p| format!("{} ({})", p.display_name, p.email))
        .unwrap_or_default();
    let email = fields.get("email").cloned().unwrap_or_else(|| {
        profile
            .as_ref()
            .map(|p| p.email.clone())
            .unwrap_or_default()
    });
    let name = fields.get("display_name").cloned().unwrap_or_else(|| {
        profile
            .as_ref()
            .map(|p| p.display_name.clone())
            .unwrap_or_default()
    });
    let error_for = |label: &str| {
        if error.to_lowercase().starts_with(&label.to_lowercase()) {
            error.clone()
        } else {
            String::new()
        }
    };
    let email_error = error_for("Email:");
    let name_error = error_for("Display name:");
    let password_error = error_for("Password:");
    let confirmation_error = error_for("Password confirmation:");
    let current_error = error_for("Current password:");
    let login = path == "/auth/login" || (path == "/auth" && !signed_in);
    let register = path == "/auth/register";
    let settings = path == "/auth/settings";
    let approval = path == "/auth" && signed_in && !code.is_empty() && code_error.is_empty();
    let dashboard = path == "/auth" && signed_in && code.is_empty();
    let known = matches!(
        path.as_str(),
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
    let session_rows: Vec<(String, String)> = sessions
        .into_iter()
        .map(|s| {
            (
                s.id,
                format!(
                    "{} session · created {}{}",
                    if s.kind == "browser" {
                        "Browser"
                    } else {
                        "Block app"
                    },
                    session_age(s.created),
                    if s.current { " · this browser" } else { "" }
                ),
            )
        })
        .collect();
    let session_csrf = store(csrf.clone());
    let csrf = store(csrf);
    let code = store(code);
    let error = store(error);
    let code_error = store(code_error);
    let path = store(path);
    let email = store(email);
    let name = store(name);
    let mail_token = store(mail_token);
    let email_error = store(email_error);
    let name_error = store(name_error);
    let password_error = store(password_error);
    let confirmation_error = store(confirmation_error);
    let current_error = store(current_error);

    view! {
        <Shell>
            <Static condition={!owned(error).is_empty()}>
                <html::Section role="alert">
                    <html::H2>"Please check your submission"</html::H2>
                    <html::P>{owned(error)}</html::P>
                </html::Section>
            </Static>
            <Static condition={!owned(code_error).is_empty()}>
                <html::P role="alert">{owned(code_error)}</html::P>
            </Static>
            <Static condition={signed_in}>
                <html::P>{format!("Signed in as {account_name}")}</html::P>
                <html::Nav>
                    <html::A href="/auth/settings">"Account settings"</html::A>
                    " · "
                    <html::A href="/auth/sessions">"Sessions"</html::A>
                </html::Nav>
            </Static>
            <Static condition={!allow_login}>
                <html::P>
                    "This server is not accepting new sign-ins. Existing sessions can still manage their account."
                </html::P>
            </Static>
            <Static condition={login && allow_login}>
                <html::H1>"Sign in"</html::H1>
                <Form action="/auth/login" csrf={owned(csrf)} code={owned(code)}>
                    <Field
                        name="email"
                        label="Email"
                        kind="email"
                        autocomplete="username"
                        value={owned(email)}
                        error={owned(email_error)}
                    />
                    <Field
                        name="password"
                        label="Password"
                        kind="password"
                        autocomplete="current-password"
                    />
                    <html::Button>"Sign in"</html::Button>
                </Form>
                <html::P>
                    <html::A href={link("/auth/forgot", &owned(code))}>
                        "Forgot your password?"
                    </html::A>
                </html::P>
                <Static condition={allow_registration}>
                    <html::A href={link("/auth/register", &owned(code))}>
                        "Create an account"
                    </html::A>
                </Static>
            </Static>
            <Static condition={register && allow_login && allow_registration}>
                <html::H1>"Create an account"</html::H1>
                <html::P>"Verify your email address to finish creating your account."</html::P>
                <Form action="/auth/register" csrf={owned(csrf)} code={owned(code)}>
                    <Field
                        name="display_name"
                        label="Display name"
                        autocomplete="nickname"
                        value={owned(name)}
                        error={owned(name_error)}
                    />
                    <Field
                        name="email"
                        label="Email"
                        kind="email"
                        autocomplete="username"
                        value={owned(email)}
                        error={owned(email_error)}
                    />
                    <Field
                        name="password"
                        label="Password (at least 8 characters)"
                        kind="password"
                        autocomplete="new-password"
                        error={owned(password_error)}
                    />
                    <Field
                        name="confirmation"
                        label="Confirm password"
                        kind="password"
                        autocomplete="new-password"
                        error={owned(confirmation_error)}
                    />
                    <html::Button>"Create account"</html::Button>
                </Form>
                <html::A href={link("/auth/login", &owned(code))}>
                    "Already have an account? Sign in"
                </html::A>
            </Static>
            <Static condition={register && !allow_registration}>
                <html::P>"This server is not accepting new accounts."</html::P>
            </Static>
            <Static condition={owned(path) == "/auth/forgot" && allow_login}>
                <html::H1>"Reset your password"</html::H1>
                <html::P>
                    "This resets your sign-in password. Encrypted data still requires your recovery phrase or another unlocked device in Block."
                </html::P>
                <Form action="/auth/forgot" csrf={owned(csrf)} code={owned(code)}>
                    <Field
                        name="email"
                        label="Email"
                        kind="email"
                        autocomplete="username"
                        value={owned(email)}
                    />
                    <html::Button>"Send reset link"</html::Button>
                </Form>
                <html::A href={link("/auth/login", &owned(code))}>"Back to sign in"</html::A>
            </Static>
            <Static condition={owned(path) == "/auth/reset" && allow_login}>
                <html::H1>"Choose a new password"</html::H1>
                <html::P>
                    "All existing sessions will be signed out. Your encryption keys stay unchanged."
                </html::P>
                <Form action="/auth/reset" csrf={owned(csrf)} code={owned(code)}>
                    <html::Input r#type="hidden" name="token" value={owned(mail_token)} />
                    <Field
                        name="password"
                        label="New password (at least 8 characters)"
                        kind="password"
                        autocomplete="new-password"
                        error={owned(password_error)}
                    />
                    <Field
                        name="confirmation"
                        label="Confirm password"
                        kind="password"
                        autocomplete="new-password"
                        error={owned(confirmation_error)}
                    />
                    <html::Button>"Reset password"</html::Button>
                </Form>
            </Static>
            <Static condition={owned(path) == "/auth/verify"}>
                <html::H1>"Verify your email address"</html::H1>
                <Form action="/auth/verify" csrf={owned(csrf)} code={owned(code)}>
                    <html::Input r#type="hidden" name="token" value={owned(mail_token)} />
                    <html::Button>"Verify email"</html::Button>
                </Form>
            </Static>
            <Static condition={approval && allow_login}>
                <html::H1>"Approve sign-in to Block"</html::H1>
                <html::P>
                    "Only continue if you started this sign-in. Check that this code matches the code in Block."
                </html::P>
                <html::P>
                    <html::Strong>{owned(code)}</html::Strong>
                </html::P>
                <Form action="/auth/approve" csrf={owned(csrf)} code={owned(code)}>
                    <html::Button>"Approve sign-in"</html::Button>
                </Form>
                <Form action="/auth/deny" csrf={owned(csrf)} code={owned(code)}>
                    <html::Button>"Deny"</html::Button>
                </Form>
            </Static>
            <Static condition={dashboard}>
                <html::H1>"Your account"</html::H1>
                <html::P>
                    "Manage your account or enter a code from Block to sign in on another device."
                </html::P>
            </Static>
            <Static condition={owned(path) == "/auth" || owned(path) == "/auth/login"}>
                <html::H2>"Have a sign-in code?"</html::H2>
                <Form action="/auth/code" csrf={owned(csrf)} code="">
                    <Field
                        name="code"
                        label="Code shown in Block"
                        value={owned(code)}
                        autocomplete="off"
                    />
                    <html::Button>"Continue with code"</html::Button>
                </Form>
            </Static>
            <Static condition={settings}>
                <html::H1>"Account settings"</html::H1>
                <Static condition={saved}>
                    <html::P role="status">"Changes saved."</html::P>
                </Static>
                <html::H2>"Display name"</html::H2>
                <Form action="/auth/settings/name" csrf={owned(csrf)} code="">
                    <Field
                        name="display_name"
                        label="Display name"
                        value={owned(name)}
                        autocomplete="nickname"
                        error={owned(name_error)}
                    />
                    <html::Button>"Save display name"</html::Button>
                </Form>
                <html::H2>"Email address"</html::H2>
                <html::P>"Your address changes after you verify the new address."</html::P>
                <Form action="/auth/settings/email" csrf={owned(csrf)} code="">
                    <Field
                        name="email"
                        label="New email"
                        kind="email"
                        value={owned(email)}
                        autocomplete="email"
                        error={owned(email_error)}
                    />
                    <Field
                        name="current_password"
                        label="Current password"
                        kind="password"
                        autocomplete="current-password"
                        error={owned(current_error)}
                    />
                    <html::Button>"Verify new email"</html::Button>
                </Form>
                <html::H2>"Password"</html::H2>
                <html::P>"Changing your password signs out all other sessions."</html::P>
                <Form action="/auth/settings/password" csrf={owned(csrf)} code="">
                    <Field
                        name="current_password"
                        label="Current password"
                        kind="password"
                        autocomplete="current-password"
                        error={owned(current_error)}
                    />
                    <Field
                        name="password"
                        label="New password (at least 8 characters)"
                        kind="password"
                        autocomplete="new-password"
                        error={owned(password_error)}
                    />
                    <Field
                        name="confirmation"
                        label="Confirm password"
                        kind="password"
                        autocomplete="new-password"
                        error={owned(confirmation_error)}
                    />
                    <html::Button>"Change password"</html::Button>
                </Form>
            </Static>
            <Static condition={owned(path) == "/auth/sessions"}>
                <html::H1>"Signed-in sessions"</html::H1>
                <html::P>
                    "Revoking a session disconnects it from this server. It does not erase data already saved on that device."
                </html::P>
                <ForEach keys={session_rows}>|(id, description): (String, String)|
                    <html::Section>
                        <html::P>{description}</html::P>
                        <Form action="/auth/sessions/revoke" csrf={owned(session_csrf)} code="">
                            <html::Input r#type="hidden" name="session" value={id} />
                            <html::Button>"Sign out session"</html::Button>
                        </Form>
                    </html::Section>
                </ForEach>
                <Form action="/auth/sessions/others" csrf={owned(csrf)} code="">
                    <html::Button>"Sign out all other sessions"</html::Button>
                </Form>
            </Static>
            <Static condition={owned(path) == "/auth/approved"}>
                <html::H1>"Sign-in approved"</html::H1>
                <html::P>
                    "Return to Block to unlock your encrypted data. You can close this page."
                </html::P>
            </Static>
            <Static condition={owned(path) == "/auth/denied"}>
                <html::H1>"Sign-in denied"</html::H1>
                <html::P>"The requesting device has not been signed in."</html::P>
            </Static>
            <Static condition={owned(path) == "/auth/check-email"}>
                <html::H1>"Check your email"</html::H1>
                <html::P>
                    "Follow the verification link to continue. Email delivery is currently recorded in the server log; ask your server operator for the message."
                </html::P>
                <html::P>"If the link expires, submit the form again to request another."</html::P>
                <html::A
                    href={link(if signed_in { "/auth/settings" } else { "/auth/register" }, &owned(code))}
                >
                    "Request another verification link"
                </html::A>
            </Static>
            <Static condition={owned(path) == "/auth/reset-requested"}>
                <html::H1>"Check your email"</html::H1>
                <html::P>
                    "If an account matches that address, a reset link has been requested. Email delivery is currently recorded in the server log; ask your server operator for the message."
                </html::P>
            </Static>
            <Static condition={owned(path) == "/auth/password-reset"}>
                <html::H1>"Password reset"</html::H1>
                <html::A href={link("/auth/login", &owned(code))}>
                    "Sign in with your new password"
                </html::A>
            </Static>
            <Static condition={owned(path) == "/auth/email-updated"}>
                <html::H1>"Email verified"</html::H1>
                <html::P>"Your email address has been updated."</html::P>
                <html::A href="/auth">"Back to account"</html::A>
            </Static>
            <Static condition={!known}>
                <html::H1>"Page not found"</html::H1>
                <html::A href="/auth">"Open the account portal"</html::A>
            </Static>
            <Static condition={signed_in}>
                <Form action="/auth/logout" csrf={owned(csrf)} code={owned(code)}>
                    <html::Button>"Sign out / use another account"</html::Button>
                </Form>
            </Static>
        </Shell>
    }
}

#[cfg(test)]
mod tests;
