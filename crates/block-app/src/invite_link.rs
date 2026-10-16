use be_keys::InviteCode;
use uuid::Uuid;

const MARKER: &str = "#invite/";

pub(crate) fn link(base: &str, code: &InviteCode) -> String {
    let base = base.split('#').next().unwrap_or_default();
    format!(
        "{}/{MARKER}{}",
        base.trim_end_matches('/'),
        code.to_string().replace('-', "")
    )
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InviteLink {
    pub(crate) server: Option<String>,
    pub(crate) code: InviteCode,
}

impl InviteLink {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (server, code) = match text.split_once(MARKER) {
            Some((server, code)) => {
                let server = server.trim_end_matches('/');
                (
                    Some(server.to_owned()).filter(|server| !server.is_empty()),
                    code,
                )
            }
            None => (None, text),
        };
        Some(Self {
            server,
            code: InviteCode::parse(code).ok()?,
        })
    }

    pub(crate) fn invitation(&self) -> Uuid {
        Uuid::from_bytes(self.code.lookup())
    }

    pub(crate) fn is_for(&self, base: &str) -> bool {
        let base = base.split('#').next().unwrap_or_default();
        self.server
            .as_deref()
            .is_none_or(|server| server == base.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests;
