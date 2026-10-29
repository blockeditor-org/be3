use be_keys::InviteCode;

const INVITE: &str = "#invite/";
const CONTACT: &str = "#contact/";

pub fn link(base: &str, code: &InviteCode) -> String {
    format_link(base, INVITE, code)
}

pub fn contact_link(base: &str, code: &InviteCode) -> String {
    format_link(base, CONTACT, code)
}

fn format_link(base: &str, marker: &str, code: &InviteCode) -> String {
    let base = base.split('#').next().unwrap_or_default();
    format!(
        "{}/{marker}{}",
        base.trim_end_matches('/'),
        code.to_string().replace('-', "")
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct InviteLink {
    pub server: Option<String>,
    pub code: InviteCode,
}

impl InviteLink {
    pub fn parse(text: &str) -> Option<Self> {
        Self::parse_marked(text, INVITE)
    }

    pub fn parse_contact(text: &str) -> Option<Self> {
        Self::parse_marked(text, CONTACT)
    }

    fn parse_marked(text: &str, marker: &str) -> Option<Self> {
        let text = text.trim();
        let (server, code) = match text.split_once(marker) {
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

    pub fn is_for(&self, base: &str) -> bool {
        let base = base.split('#').next().unwrap_or_default();
        self.server
            .as_deref()
            .is_none_or(|server| server == base.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests;
