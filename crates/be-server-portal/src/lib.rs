use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    #[default]
    SignIn,
    SignInClosed,
    Register,
    RegistrationClosed,
    Verify,
    /// An operator's login link, waiting for the person to use it.
    LoginLink,
    Approve,
    Account,
    Settings,
    Sessions,
    Approved,
    Denied,
    CheckEmail,
    EmailUpdated,
    /// Asks for the code mailed for the browser's attempt.
    EnterCode,
    /// The mail's link, opened in a browser that didn't ask for the code: it shows the code
    /// and signs nothing in.
    CodeElsewhere {
        code: String,
    },
    /// "I can't sign in": a code to the account's email that ends every other session.
    Recover,
    /// A provider sign-in with no account, waiting for the operator's invite code.
    ProviderRegister,
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
    Host,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub kind: SessionKind,
    pub age: String,
    pub current: bool,
}

/// A link that begins signing in with a provider (`/auth/oidc/{id}/start`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderLink {
    pub name: String,
    pub href: String,
}

/// What a passkey ceremony a page offers is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PasskeyStep {
    /// Signing in with any passkey the browser has for this server, with no email first.
    SignIn,
    /// Confirming it is the signed-in account's owner with one of its passkeys.
    Proof,
    /// Adding a passkey to the signed-in account.
    Add,
}

impl PasskeyStep {
    /// Where the browser's answer is posted.
    pub fn action(self) -> &'static str {
        match self {
            PasskeyStep::SignIn => "/auth/passkey/sign-in",
            PasskeyStep::Proof => "/auth/passkey/proof",
            PasskeyStep::Add => "/auth/passkey/add",
        }
    }

    /// Whether the browser makes a passkey (`navigator.credentials.create`) rather than using
    /// one (`get`).
    pub fn creates(self) -> bool {
        self == PasskeyStep::Add
    }
}

/// A passkey ceremony the page began, bound to its form token.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PasskeyOffer {
    pub step: PasskeyStep,
    /// The `publicKey` options of the browser's call, as WebAuthn's JSON
    /// (`PublicKeyCredential.parseRequestOptionsFromJSON` or `parseCreationOptionsFromJSON`).
    pub options: String,
}

/// A way to sign in added to or removed from the account, as the settings page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdentityChange {
    /// What changed: "Added Google (ada@example.com)", say.
    pub text: String,
    /// When, to the minute in UTC.
    pub when: String,
}

/// A way to sign in to the account, as the settings page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdentityRow {
    pub id: String,
    /// `email`, `oidc` or `passkey`.
    pub kind: String,
    /// The email, the provider's name and the email it gave, or when the passkey was added.
    pub label: String,
    /// False for the account's last identity, which can't be removed.
    pub removable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Field {
    Email,
    DisplayName,
    Invite,
    EmailCode,
}

impl Field {
    pub const ALL: [Field; 4] = [
        Field::Email,
        Field::DisplayName,
        Field::Invite,
        Field::EmailCode,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Field::Email => "email",
            Field::DisplayName => "display_name",
            Field::Invite => "invite",
            Field::EmailCode => "email_code",
        }
    }

    fn prefix(&self) -> &'static str {
        match self {
            Field::Email => "Email:",
            Field::DisplayName => "Display name:",
            Field::Invite => "Invite code:",
            Field::EmailCode => "Code:",
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
    /// On the approve screen, where the device asking to sign in connected from: an address
    /// the server saw, not anything the device said of itself.
    pub requester: String,
    pub csrf: String,
    pub problem: Option<Problem>,
    pub email: String,
    pub display_name: String,
    /// The invite code typed into the registration form, kept when it is shown again.
    pub invite: String,
    pub mail_token: String,
    pub sessions: Vec<Session>,
    /// The account's ways to sign in, on the settings page.
    pub identities: Vec<IdentityRow>,
    /// Ways to sign in added or removed in the last thirty days, newest first, on the settings
    /// page.
    pub identity_changes: Vec<IdentityChange>,
    /// Whether this browser proved it is the owner recently enough for account changes, and to
    /// approve a sign-in.
    pub fresh: bool,
    /// Where the browser's pending code went.
    pub code_email: String,
    /// A code the mail's link filled in, in the browser that asked for it.
    pub email_code: String,
    pub saved: bool,
    pub allow_registration: bool,
    /// Providers to sign in with, on the sign-in and registration pages, or to add, on the
    /// settings page.
    pub providers: Vec<ProviderLink>,
    /// The account's own providers it may prove it is the owner with, on the settings page.
    pub proof_providers: Vec<ProviderLink>,
    /// The provider a screen is about: the one waiting for an invite code, or the one a code
    /// to the account's email will link.
    pub provider: String,
    /// The passkey ceremony the page offers, if the server has an RP ID and the screen has one.
    pub passkey: Option<PasskeyOffer>,
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
