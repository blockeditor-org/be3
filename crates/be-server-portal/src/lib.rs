use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    #[default]
    SignIn,
    SignInClosed,
    Register,
    RegistrationClosed,
    Forgot,
    Reset,
    Verify,
    Approve,
    Account,
    Settings,
    Sessions,
    Approved,
    Denied,
    CheckEmail,
    ResetRequested,
    PasswordReset,
    EmailUpdated,
    NotFound,
    Message {
        title: String,
        text: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub display_name: String,
    pub email: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionKind {
    Browser,
    App,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub kind: SessionKind,
    pub age: String,
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Field {
    Email,
    DisplayName,
    Password,
    Confirmation,
    CurrentPassword,
}

impl Field {
    pub const ALL: [Field; 5] = [
        Field::Email,
        Field::DisplayName,
        Field::Password,
        Field::Confirmation,
        Field::CurrentPassword,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Field::Email => "email",
            Field::DisplayName => "display_name",
            Field::Password => "password",
            Field::Confirmation => "confirmation",
            Field::CurrentPassword => "current_password",
        }
    }

    fn prefix(&self) -> &'static str {
        match self {
            Field::Email => "Email:",
            Field::DisplayName => "Display name:",
            Field::Password => "Password:",
            Field::Confirmation => "Password confirmation:",
            Field::CurrentPassword => "Current password:",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub field: Option<Field>,
    pub message: String,
}

impl Problem {
    pub fn from_message(message: &str) -> Self {
        let field = Field::ALL
            .into_iter()
            .filter(|field| {
                message
                    .to_lowercase()
                    .starts_with(&field.prefix().to_lowercase())
            })
            .max_by_key(|field| field.prefix().len());
        let message = match &field {
            Some(field) => sentence(message[field.prefix().len()..].trim()),
            None => sentence(message),
        };
        Self { field, message }
    }
}

fn sentence(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub screen: Screen,
    pub account: Option<Account>,
    pub code: String,
    pub code_problem: String,
    pub csrf: String,
    pub problem: Option<Problem>,
    pub email: String,
    pub display_name: String,
    pub mail_token: String,
    pub sessions: Vec<Session>,
    pub saved: bool,
    pub allow_registration: bool,
    pub assets: String,
}

impl Page {
    pub fn problem_for(&self, field: Field) -> String {
        match &self.problem {
            Some(Problem {
                field: Some(found),
                message,
            }) if *found == field => message.clone(),
            _ => String::new(),
        }
    }

    pub fn general_problem(&self) -> String {
        match &self.problem {
            Some(Problem {
                field: None,
                message,
            }) => message.clone(),
            _ => String::new(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .expect("a page always serializes")
            .replace('<', "\\u003c")
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|error| error.to_string())
    }
}

pub fn encode(value: &str) -> String {
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

pub fn with_code(path: &str, code: &str) -> String {
    format!("{path}?code={}", encode(code))
}

pub const PAGE_ID: &str = "be-portal-page";

pub const ROOT_ID: &str = "be-portal";

pub struct Asset {
    pub name: &'static str,
    pub content_type: &'static str,
    pub bytes: &'static [u8],
}

#[derive(Clone, Copy)]
pub struct Portal {
    pub render: fn(&Page) -> String,
    pub import_map: fn(&str) -> String,
    pub assets: &'static [Asset],
}

impl std::fmt::Debug for Portal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Portal")
            .field("assets", &self.assets.len())
            .finish()
    }
}

#[cfg(test)]
mod tests;
