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

pub(crate) fn page(services: &Services, code: &str, error: &str) -> Result<String, ServerError> {
    let csrf = services.browser_auth.form()?;
    let login = if services.config.allow_login {
        "<button name=action value=login>Sign in and authorize</button>"
    } else {
        ""
    };
    let register = if services.config.allow_registration {
        "<label>Display name (for a new account)<input name=display_name autocomplete=nickname maxlength=200></label><button name=action value=register>Create account and authorize</button>"
    } else {
        ""
    };
    Ok(shell(&format!(
        r#"<h1>Authorize Block</h1><p>Only continue if you started adding an account on your device. Check that its code matches.</p><p role=alert>{error}</p><form method=post action=/auth><input type=hidden name=csrf value="{csrf}"><label>Device code<input name=code value="{code}" required autocomplete=off maxlength=14></label><label>Email<input name=email type=email autocomplete=username></label><label>Password<input name=password type=password autocomplete=current-password></label>{login}{register}<button name=action value=deny formnovalidate>Deny</button></form>"#,
        error = escape(error),
        code = escape(code)
    )))
}

pub(crate) fn shell(content: &str) -> String {
    format!(
        r#"<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content="width=device-width, initial-scale=1"><title>Block — authorize device</title><style>body{{font:1rem system-ui;margin:3rem auto;padding:0 1rem;max-width:28rem;color:#202124;background:#fafafa}}label{{display:block;margin:1rem 0}}input{{display:block;box-sizing:border-box;width:100%;padding:.7rem;font:inherit}}button{{padding:.7rem;margin:.4rem .4rem .4rem 0;font:inherit;cursor:pointer}}[role=alert]{{color:#a00}}</style>{content}</html>"#
    )
}

pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
