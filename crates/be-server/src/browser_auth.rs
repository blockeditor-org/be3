use beui_html::elements as html;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use be_protocol::{ErrorCode, ServerMessage};
use tokio::time::Instant;
use uuid::Uuid;

use crate::{ServerError, Services, watch::WatchHub};

const LIFETIME: Duration = Duration::from_secs(600);
const LIMIT: usize = 1024;

#[derive(Default)]
pub(crate) struct Sessions {
    pending: Mutex<HashMap<String, Pending>>,
    forms: Mutex<HashMap<String, Instant>>,
}

struct Pending {
    client: u64,
    secret: String,
    denied: bool,
    expires: Instant,
    account: Option<Uuid>,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

impl Sessions {
    pub(crate) fn begin(
        self: &Arc<Self>,
        client: u64,
        hub: Arc<WatchHub>,
    ) -> Result<(String, String), ServerError> {
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, entry| entry.client != client && entry.expires > Instant::now());
        if pending.len() >= LIMIT {
            return Err(invalid("too many pending sign-ins; try again later"));
        }
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let secret = Uuid::new_v4().to_string();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        pending.insert(
            code.clone(),
            Pending {
                client,
                secret: secret.clone(),
                denied: false,
                expires: Instant::now() + LIFETIME,
                account: None,
                _cancel: cancel,
            },
        );
        let sessions = Arc::downgrade(self);
        let expiring = code.clone();
        tokio::spawn(async move {
            tokio::select! { _ = tokio::time::sleep(LIFETIME) => {}, _ = cancelled => return }
            if let Some(sessions) = sessions.upgrade() {
                let expired = sessions.pending.lock().unwrap().remove(&expiring).is_some();
                if expired {
                    hub.send_to(
                        client,
                        ServerMessage::BrowserLoginFinished {
                            approved: false,
                            message: "This sign-in expired. Try again.".into(),
                        },
                    )
                    .await;
                }
            }
        });
        Ok((code, secret))
    }

    pub(crate) fn finish(
        &self,
        client: u64,
        code: &str,
        secret: &str,
    ) -> Result<Option<Uuid>, ServerError> {
        let mut pending = self.pending.lock().unwrap();
        let entry = pending
            .get_mut(code)
            .filter(|entry| entry.secret == secret && entry.expires > Instant::now())
            .ok_or_else(|| invalid("this sign-in expired or was cancelled"))?;
        if entry.denied {
            return Err(invalid("Sign-in was denied in the browser."));
        }
        entry.client = client;
        let Some(account) = entry.account else {
            return Ok(None);
        };
        pending.remove(code);
        Ok(Some(account))
    }

    fn form(&self) -> Result<String, ServerError> {
        let mut forms = self.forms.lock().unwrap();
        forms.retain(|_, expires| *expires > Instant::now());
        if forms.len() >= LIMIT {
            return Err(invalid("too many open sign-in pages; try again later"));
        }
        let token = Uuid::new_v4().to_string();
        forms.insert(token.clone(), Instant::now() + LIFETIME);
        Ok(token)
    }

    fn consume_form(&self, token: &str) -> Result<(), ServerError> {
        self.forms
            .lock()
            .unwrap()
            .remove(token)
            .filter(|expires| *expires > Instant::now())
            .ok_or_else(|| invalid("this page expired; reload it and try again"))?;
        Ok(())
    }
}

use beui_html::{Children, Node, Show, Stylesheet, component, render, view};

pub(crate) fn page(services: &Services, code: &str, error: &str) -> Result<String, ServerError> {
    let csrf = services.browser_auth.form()?;
    let allow_login = services.config.allow_login;
    let allow_registration = services.config.allow_registration;
    let code = code.to_string();
    let error = error.to_string();
    Ok(render(|| {
        view! {
            <Shell>
                <html::Main>
                    <html::H1>"Authorize Block"</html::H1>
                    <html::P>
                        "Only continue if you started adding an account on your device. Check that its code matches."
                    </html::P>
                    <html::P role="alert">{error}</html::P>
                    <html::Form method="post" action="/auth">
                        <html::Input r#type="hidden" name="csrf" value={csrf} />
                        <html::Label>
                            "Device code"
                            <html::Input
                                name="code"
                                value={code}
                                required=true
                                autocomplete="off"
                                maxlength=14
                            />
                        </html::Label>
                        <html::Label>
                            "Email"
                            <html::Input name="email" r#type="email" autocomplete="username" />
                        </html::Label>
                        <html::Label>
                            "Password"
                            <html::Input
                                name="password"
                                r#type="password"
                                autocomplete="current-password"
                            />
                        </html::Label>
                        <Show condition={allow_login}>
                            <html::Button name="action" value="login">
                                "Sign in and authorize"
                            </html::Button>
                        </Show>
                        <Show condition={allow_registration}>
                            <Registration />
                        </Show>
                        <html::Button name="action" value="deny" formnovalidate=true>
                            "Deny"
                        </html::Button>
                    </html::Form>
                </html::Main>
            </Shell>
        }
    }))
}

#[component]
fn Shell(children: Children<Node>) -> Node {
    view! {
        <html::Html lang="en">
            <html::Head>
                <html::Meta charset="utf-8" />
                <html::Meta name="viewport" content="width=device-width, initial-scale=1" />
                <html::Title>"Block — authorize device"</html::Title>
                <Stylesheet css={include_str!("browser_auth.css")} />
            </html::Head>
            <html::Body>{children}</html::Body>
        </html::Html>
    }
}

#[component]
fn Registration() -> Children<Node> {
    view! {
        <html::Label>
            "Display name (for a new account)"
            <html::Input name="display_name" autocomplete="nickname" maxlength=200 />
        </html::Label>
        <html::Button name="action" value="register">"Create account and authorize"</html::Button>
    }
}

pub(crate) fn message(title: &str, message: &str) -> String {
    render(|| {
        view! {
            <Shell>
                <html::H1>{title}</html::H1>
                <html::P>{message}</html::P>
            </Shell>
        }
    })
}

pub(crate) fn fields(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((decode(key)?, decode(value)?))
        })
        .collect()
}

fn decode(value: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut source = value.bytes();
    while let Some(byte) = source.next() {
        bytes.push(match byte {
            b'+' => b' ',
            b'%' => u8::from_str_radix(
                &String::from_utf8(vec![source.next()?, source.next()?]).ok()?,
                16,
            )
            .ok()?,
            other => other,
        });
    }
    String::from_utf8(bytes).ok()
}

pub(crate) async fn submit(
    services: &Services,
    fields: &HashMap<String, String>,
) -> Result<(), ServerError> {
    let field = |key: &str| fields.get(key).map(String::as_str).unwrap_or_default();
    services.browser_auth.consume_form(field("csrf"))?;
    let code = field("code").trim().to_ascii_lowercase();
    {
        let pending = services.browser_auth.pending.lock().unwrap();
        let entry = pending
            .get(&code)
            .filter(|entry| {
                entry.expires > Instant::now() && entry.account.is_none() && !entry.denied
            })
            .ok_or_else(|| invalid("this code is invalid, expired, or already approved"))?;
        let _ = entry;
    }
    if field("action") == "deny" {
        let entry = {
            let mut pending = services.browser_auth.pending.lock().unwrap();
            pending.get_mut(&code).map(|entry| {
                entry.denied = true;
                entry.client
            })
        };
        if let Some(entry) = entry {
            services
                .hub
                .send_to(
                    entry,
                    ServerMessage::BrowserLoginFinished {
                        approved: false,
                        message: "Sign-in was denied in the browser.".into(),
                    },
                )
                .await;
        }
        return Ok(());
    }
    let (profile, token) = match field("action") {
        "login" if services.config.allow_login => {
            services
                .store
                .login(field("email"), field("password"))
                .await?
        }
        "register" if services.config.allow_registration => {
            services
                .store
                .register(field("email"), field("display_name"), field("password"))
                .await?
        }
        _ => return Err(invalid("this server does not allow that sign-in method")),
    };
    services.store.logout(&token).await?;
    let client = {
        let mut pending = services.browser_auth.pending.lock().unwrap();
        let entry = pending
            .get_mut(&code)
            .filter(|entry| {
                entry.expires > Instant::now() && entry.account.is_none() && !entry.denied
            })
            .ok_or_else(|| invalid("this sign-in expired or was cancelled"))?;
        entry.account = Some(profile.account);
        entry.client
    };
    services
        .hub
        .send_to(
            client,
            ServerMessage::BrowserLoginFinished {
                approved: true,
                message: String::new(),
            },
        )
        .await;
    Ok(())
}

#[cfg(test)]
mod tests;
