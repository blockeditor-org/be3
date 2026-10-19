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
    ) -> Result<String, ServerError> {
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, entry| entry.client != client && entry.expires > Instant::now());
        if pending.len() >= LIMIT {
            return Err(invalid("too many pending sign-ins; try again later"));
        }
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        pending.insert(
            code.clone(),
            Pending {
                client,
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
        Ok(code)
    }

    pub(crate) fn cancel(&self, client: u64) {
        self.pending
            .lock()
            .unwrap()
            .retain(|_, entry| entry.client != client);
    }

    pub(crate) fn finish(&self, client: u64, code: &str) -> Result<Uuid, ServerError> {
        let mut pending = self.pending.lock().unwrap();
        let entry = pending
            .get(code)
            .filter(|entry| entry.client == client && entry.expires > Instant::now())
            .ok_or_else(|| invalid("this sign-in expired or was cancelled"))?;
        let account = entry
            .account
            .ok_or_else(|| invalid("approve this sign-in in the browser first"))?;
        pending.remove(code);
        Ok(account)
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

use beui_html::{Element, Node, attr, document, fragment, render, stylesheet, view};

pub(crate) fn page(services: &Services, code: &str, error: &str) -> Result<String, ServerError> {
    let csrf = services.browser_auth.form()?;
    let allow_login = services.config.allow_login;
    let allow_registration = services.config.allow_registration;
    let code = code.to_string();
    let error = error.to_string();
    Ok(render(|| {
        let login = if allow_login {
            Node::element("button")
                .attr("name", "action")
                .attr("value", "login")
                .child("Sign in and authorize")
        } else {
            fragment([])
        };
        let register = if allow_registration {
            fragment([
                Node::element("label")
                    .child("Display name (for a new account)")
                    .child(
                        Node::element("input")
                            .attr("name", "display_name")
                            .attr("autocomplete", "nickname")
                            .attr("maxlength", "200"),
                    ),
                Node::element("button")
                    .attr("name", "action")
                    .attr("value", "register")
                    .child("Create account and authorize"),
            ])
        } else {
            fragment([])
        };
        shell_node(view! {
            <Element tag="main">
                <Element tag="h1">"Authorize Block"</Element>
                <Element tag="p">
                    "Only continue if you started adding an account on your device. Check that its code matches."
                </Element>
                <Element tag="p" @attr={("role", "alert")}>{error}</Element>
                <Element tag="form" @attr={("method", "post")} @attr={("action", "/auth")}>
                    {Node::element("input").attr("type", "hidden").attr("name", "csrf").attr("value", csrf)}
                    <Element tag="label">
                        "Device code"
                        {Node::element("input").attr("name", "code").attr("value", code).flag("required", true).attr("autocomplete", "off").attr("maxlength", "14")}
                    </Element>
                    <Element tag="label">
                        "Email"
                        {Node::element("input").attr("name", "email").attr("type", "email").attr("autocomplete", "username")}
                    </Element>
                    <Element tag="label">
                        "Password"
                        {Node::element("input").attr("name", "password").attr("type", "password").attr("autocomplete", "current-password")}
                    </Element>
                    {login}
                    {register}
                    {Node::element("button").attr("name", "action").attr("value", "deny").flag("formnovalidate", true).child("Deny")}
                </Element>
            </Element>
        })
    }))
}

fn shell_node(content: Node) -> Node {
    Node::element("html")
        .attr("lang", "en")
        .child(
            Node::element("head")
                .child(Node::element("meta").attr("charset", "utf-8"))
                .child(
                    Node::element("meta")
                        .attr("name", "viewport")
                        .attr("content", "width=device-width, initial-scale=1"),
                )
                .child(Node::element("title").child("Block — authorize device"))
                .child(stylesheet(include_str!("browser_auth.css"))),
        )
        .child(Node::element("body").child(content))
}

pub(crate) fn message(title: &str, message: &str) -> String {
    document(shell_node(fragment([
        Node::element("h1").child(title),
        Node::element("p").child(message),
    ])))
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
            .filter(|entry| entry.expires > Instant::now() && entry.account.is_none())
            .ok_or_else(|| invalid("this code is invalid, expired, or already approved"))?;
        let _ = entry;
    }
    if field("action") == "deny" {
        let entry = services.browser_auth.pending.lock().unwrap().remove(&code);
        if let Some(entry) = entry {
            services
                .hub
                .send_to(
                    entry.client,
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
            .filter(|entry| entry.expires > Instant::now() && entry.account.is_none())
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
