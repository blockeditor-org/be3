//! The host's block stack as a root guest shows it (plans/host_as_platform_adapter.md, 4g7b2):
//! `be3:root`'s `core-view`, filled from `HostCore` as be-host's own UI reads it
//! (`HostCore::view`), with what that view words for the person given as plain data; the
//! root's `ask-core` commands, done as be-host's own UI sends them (`UiCommand`); and its
//! `make-secret` requests, answered once `HostCore` made the secret and then let go of on the
//! host's side, since the root holds it while it shows it.

use be_wasm_password::Password;
use be_wasm_root_api::{
    AccountRow, AgentCredentials, AgentRow, AgentsArea, BrowserSignIn, CoreCommand, CoreFailure,
    CoreSecret, CoreView, CoreWait, DefaultServer, DiscardReason, HostedBlock, Invitation,
    Leaving, MemberRole, NewAccount, OrganizationRow, OrganizationsArea, OrganizationsListed,
    PairingRequest, ProfileRow, Reauthenticating, RecoveryArea, Refusal, SecretCommand,
    SessionKind, SessionRow, SessionsArea, SetAside, SigningIn, UnlockArea, UnsentEdit,
};
use be_wasm_root_host::Root;
use uuid::Uuid;

use crate::{HostCore, PendingDestructiveAction, root_settings, ui};

/// The root's `make-secret` requests the host took and has not answered yet.
#[derive(Default)]
pub(super) struct BlockStack {
    making: Vec<(u64, Making)>,
}

/// A secret `HostCore` is making for the root, which it hands over once made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Making {
    Invite,
    ContactLink,
    Agent,
    PairingCode,
}

impl BlockStack {
    /// Does what the root asked of the block stack since the last frame, answers the secrets it
    /// asked for that are made, and shows it the block stack as it now stands. The root is
    /// woken only when what it is shown changed (`Root::show_core`).
    pub(super) fn step(
        &mut self,
        core: &mut HostCore,
        root: &mut Root,
        context: &beui::native::Context,
    ) {
        for command in root.take_core_commands() {
            for command in commands(command) {
                core.command(context, command);
            }
        }
        for (id, command) in root.take_secret_requests() {
            match self.start(core, context, command) {
                Ok(making) => self.making.push((id, making)),
                Err(secret) => {
                    root.answer_secret(id, secret);
                }
            }
        }
        let mut waiting = Vec::new();
        for (id, making) in std::mem::take(&mut self.making) {
            // The agent being made, by its account, once it is.
            let agent = match making {
                Making::Agent => core.agent_created.as_ref().map(|(account, _)| *account),
                _ => None,
            };
            match made(core, making) {
                Some(secret) => {
                    // An agent's credentials cannot be had again: one nobody waits for any more
                    // is removed, as the root's `make-secret` says.
                    if !root.answer_secret(id, secret)
                        && let Some(account) = agent
                    {
                        core.command(context, ui::UiCommand::RemoveAgent(account));
                    }
                }
                None => waiting.push((id, making)),
            }
        }
        self.making = waiting;
        root.show_core(view(core));
    }

    /// Starts making the secret `command` asks for: what it is making, or at once the secret,
    /// or none if it cannot be made.
    fn start(
        &self,
        core: &mut HostCore,
        context: &beui::native::Context,
        command: SecretCommand,
    ) -> Result<Making, Option<CoreSecret>> {
        let making = match command {
            SecretCommand::InvitationLink(id) => {
                let link = core.invitations.link_of(Uuid::from_bytes(id));
                return Err(link.map(|link| CoreSecret::Link(Password::from(link))));
            }
            SecretCommand::RecoveryPhrase => {
                let words = core.keys.recovery_view().words;
                return Err(
                    (!words.is_empty()).then(|| CoreSecret::Phrase(Password::from(words.join(" "))))
                );
            }
            SecretCommand::InviteMember => {
                core.command(context, ui::UiCommand::InviteMember);
                Making::Invite
            }
            SecretCommand::ContactLink => {
                core.command(context, ui::UiCommand::NewContactLink);
                Making::ContactLink
            }
            SecretCommand::CreateAgent(name) => {
                core.agent_created = None;
                core.command(context, ui::UiCommand::CreateAgent(name));
                Making::Agent
            }
            SecretCommand::PairingCode => {
                core.command(context, ui::UiCommand::StartPairing);
                Making::PairingCode
            }
        };
        // Making one can fail at once, before anything waits on it.
        match made(core, making) {
            Some(secret) => Err(secret),
            None => Ok(making),
        }
    }
}

/// The secret `making`, once made or failed, which the host then lets go of: none while it is
/// still being made.
fn made(core: &mut HostCore, making: Making) -> Option<Option<CoreSecret>> {
    let link = |made: Result<String, String>| made.ok().map(|link| CoreSecret::Link(link.into()));
    match making {
        Making::Invite => {
            let made = core.invitations.made()?;
            core.invitations.close_invite();
            Some(link(made))
        }
        Making::ContactLink => {
            let made = core.contacts.made()?;
            core.contacts.forget_link();
            Some(link(made))
        }
        Making::Agent => {
            if let Some((_, created)) = core.agent_created.take() {
                return Some(Some(CoreSecret::Agent(AgentCredentials {
                    name: created.name.clone(),
                    bundle: Password::from(created.bundle(false)),
                    proxy_bundle: Password::from(created.bundle(true)),
                    proxy_header: Password::from(format!("Bearer {}", created.token)),
                })));
            }
            // Making it failed, why in the agents' error.
            core.pending_organization_request.is_none().then_some(None)
        }
        Making::PairingCode => {
            let unlock = core.keys.unlock_view(&core.account.name);
            match (unlock.code, unlock.error) {
                (Some(code), _) => Some(Some(CoreSecret::Code(Password::from(code)))),
                (None, Some(_)) => Some(None),
                (None, None) => None,
            }
        }
    }
}

/// What be-host's own UI sends for the root's `command`: what the root asks the person to
/// confirm first, it asks and confirms in one.
fn commands(command: CoreCommand) -> Vec<ui::UiCommand> {
    use ui::UiCommand as Ui;
    let text = |secret: Password| secret.as_str().unwrap_or_default().to_owned();
    let id = Uuid::from_bytes;
    match command {
        CoreCommand::RestoreUnsent(block) => vec![Ui::RestoreUnsent(id(block))],
        CoreCommand::DiscardUnsent(block) => {
            vec![Ui::DiscardUnsent(id(block)), Ui::ConfirmDiscardUnsent]
        }
        CoreCommand::OpenAccount(key) => vec![Ui::OpenAccount(key)],
        CoreCommand::ChooseOrganization(key) => vec![Ui::ChooseOrganization(key)],
        // A local account is only removed once confirmed, which the root has asked.
        CoreCommand::ForgetAccount(key) => vec![Ui::LogOut(key), Ui::ConfirmRemoveAccount],
        CoreCommand::AddAccount(account) => {
            let form = match account {
                NewAccount::Local(name) => ui::AccountForm {
                    remote: false,
                    remote_url: String::new(),
                    authorization: String::new(),
                    display_name: name,
                },
                NewAccount::Server(server) => ui::AccountForm {
                    remote: true,
                    remote_url: server.url,
                    authorization: server.gateway.map(text).unwrap_or_default(),
                    display_name: String::new(),
                },
            };
            vec![Ui::OpenAddAccount, Ui::SubmitAccount(form)]
        }
        CoreCommand::OpenSignInPage => vec![Ui::OpenLoginBrowser],
        CoreCommand::CheckSignIn => vec![Ui::CheckLoginNow],
        CoreCommand::CancelSignIn => vec![Ui::CloseAddAccount],
        CoreCommand::SwitchAccount => vec![Ui::SwitchAccount],
        CoreCommand::ReloadOrganizations => vec![Ui::ReloadOrganizations],
        CoreCommand::OpenOrganization(organization) => vec![Ui::OpenOrganization(id(organization))],
        CoreCommand::ChooseSessionIn(organization) => vec![Ui::ChooseSession(id(organization))],
        CoreCommand::CreateOrganization(name) => vec![Ui::CreateOrganization(name)],
        CoreCommand::OpenInvitation(link) => vec![Ui::OpenInvite(text(link))],
        CoreCommand::JoinInvited => vec![Ui::JoinInvited],
        CoreCommand::CancelJoin => vec![Ui::CancelJoin],
        CoreCommand::ChooseSession => vec![Ui::SwitchSession],
        CoreCommand::CloseSessions => vec![Ui::CloseSessions],
        CoreCommand::OpenSession(session) => vec![Ui::OpenSession(id(session))],
        CoreCommand::NewSession(kind) => vec![Ui::OpenNewSession(id(kind))],
        CoreCommand::UseProfile(profile) => vec![Ui::UseProfile(id(profile))],
        CoreCommand::NewProfile => vec![Ui::NewProfile],
        CoreCommand::Reauthenticate => vec![Ui::ReauthSubmit],
        CoreCommand::SwitchOrganization => vec![Ui::SwitchOrganization],
        CoreCommand::ConfirmDiscard => vec![Ui::Discard],
        CoreCommand::CancelDiscard => vec![Ui::CancelDiscard],
        CoreCommand::ListInvitations => vec![Ui::OpenInvitations],
        CoreCommand::RevokeInvitation(invitation) => vec![Ui::RevokeInvitation(id(invitation))],
        CoreCommand::ListMembers => vec![Ui::OpenMembers],
        CoreCommand::SetMemberRole(change) => vec![Ui::SetMemberRole(
            id(change.account),
            match change.role {
                MemberRole::Administrator => be_block_protocol::OrganizationRole::Administrator,
                MemberRole::Editor => be_block_protocol::OrganizationRole::Editor,
                MemberRole::Guest => be_block_protocol::OrganizationRole::Guest,
            },
        )],
        CoreCommand::RemoveMember(account) => {
            vec![Ui::RemoveMember(id(account)), Ui::ConfirmRemoveMember]
        }
        CoreCommand::ListAgents => vec![Ui::OpenAgents],
        CoreCommand::RemoveAgent(account) => vec![Ui::RemoveAgent(id(account))],
        // Opened as the host's own dialog does, so the area shows it.
        CoreCommand::OpenContactLink(link) => vec![Ui::OpenContacts, Ui::OpenContactLink(text(link))],
        CoreCommand::AcceptContact => vec![Ui::AcceptContact],
        CoreCommand::ForgetContactLink => vec![Ui::CloseContacts],
        CoreCommand::RemoveContact(account) => {
            vec![Ui::RemoveContact(id(account)), Ui::ConfirmRemoveContact]
        }
        CoreCommand::OpenShareLink(link) => {
            vec![Ui::OpenShareLinks, Ui::OpenShareLink(text(link))]
        }
        CoreCommand::AcceptShareLink => vec![Ui::AcceptShareLink],
        CoreCommand::ForgetShareLink => vec![Ui::CloseShareLinks],
        CoreCommand::ConfirmLeave(block) => vec![Ui::ConfirmLeave(id(block))],
        CoreCommand::CancelLeave => vec![Ui::CancelLeave],
        CoreCommand::ConfirmRecovery(words) => vec![Ui::ConfirmRecovery(
            text(words)
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        )],
        CoreCommand::NewRecoveryPhrase => vec![Ui::NewRecoveryPhrase],
        CoreCommand::StopHosting(block) => vec![Ui::StopHosting(id(block))],
        CoreCommand::CancelRecovery => vec![Ui::CancelRecovery],
        CoreCommand::UnlockWithPhrase(phrase) => vec![Ui::UnlockWithPhrase(text(phrase))],
        CoreCommand::CancelPairing => vec![Ui::CancelPairing],
        CoreCommand::ApprovePairing(approval) => {
            vec![Ui::ApprovePairing(approval.id, text(approval.code))]
        }
        CoreCommand::Restart => vec![Ui::Restart],
        CoreCommand::DeleteClientDatabase => vec![
            Ui::AskErrorAction(ui::ErrorAction::DeleteClientDatabase),
            Ui::ConfirmErrorAction,
        ],
        CoreCommand::Exit => vec![Ui::Exit],
    }
}

/// The block stack as the root shows it, from what be-host's own UI shows (`HostCore::view`)
/// and, where that words it for the person, from what it words.
fn view(core: &HostCore) -> CoreView {
    let shown = core.view();
    let organization = core.organization.as_ref();
    CoreView {
        generation: 0,
        unsent: be_host_stack::unsent()
            .into_iter()
            .map(|unsent| UnsentEdit {
                block: unsent.block.into_bytes(),
                name: unsent.name,
                offerable: unsent.offerable,
                aside: unsent.aside.map(set_aside),
                kept_edits: u32::try_from(unsent.kept_edits).unwrap_or(u32::MAX),
            })
            .collect(),
        waits_for: match shown.screen {
            ui::Screen::Error => Some(CoreWait::Failure),
            ui::Screen::Accounts => Some(CoreWait::Account),
            ui::Screen::Organizations => Some(CoreWait::Organization),
            ui::Screen::Recovery => Some(CoreWait::Recovery),
            ui::Screen::Unlock => Some(CoreWait::Unlock),
            ui::Screen::Sessions => Some(CoreWait::Session),
            ui::Screen::Organization => None,
        },
        accounts: shown
            .accounts
            .iter()
            .map(|account| AccountRow {
                key: account.key.clone(),
                name: account.name.clone(),
                email: account.email.clone(),
                server: (!account.local).then(|| account.server.clone()),
                has_last_organization: account.has_last_organization,
                current: account.current && core.signed_in,
                organizations: account.companies.clone(),
            })
            .collect(),
        signing_in: core
            .pending_account_request
            .as_ref()
            .map(|pending| SigningIn {
                browser: pending.browser.as_ref().map(|browser| BrowserSignIn {
                    url: browser.url.clone(),
                    code: browser.code.clone(),
                }),
            }),
        account_error: core.account_error.clone(),
        organizations: OrganizationsArea {
            listed: match shown.organizations.state {
                ui::OrganizationsState::Loading => OrganizationsListed::Loading,
                ui::OrganizationsState::Failed => OrganizationsListed::Failed,
                ui::OrganizationsState::Loaded => OrganizationsListed::Loaded(
                    shown
                        .organizations
                        .organizations
                        .iter()
                        .map(|(id, name)| OrganizationRow {
                            id: id.into_bytes(),
                            name: name.clone(),
                        })
                        .collect(),
                ),
            },
            joining: shown.organizations.joining.as_ref().map(|joining| Invitation {
                organization: joining.organization.clone(),
                inviter: joining.inviter.clone(),
                problem: (!joining.problem.is_empty()).then(|| joining.problem.clone()),
                can_join: joining.can_join,
            }),
            busy: shown.organizations.busy,
            error: shown.organizations.error.clone(),
        },
        sessions: SessionsArea {
            loaded: shown.status.settings_loaded,
            sessions: core
                .root_settings
                .sessions(core.client_id, true)
                .into_iter()
                .map(|session| SessionRow {
                    id: session.id.into_bytes(),
                    name: session.name,
                    session_type: session.editor.into_bytes(),
                    current: session.current,
                })
                .collect(),
            session_kinds: root_settings::SESSION_TYPES
                .iter()
                .map(|(id, name)| SessionKind {
                    id: id.into_bytes(),
                    name: (*name).to_owned(),
                })
                .collect(),
            session_type: shown.status.session_type.into_bytes(),
            profiles: shown
                .status
                .profiles
                .iter()
                .map(|profile| ProfileRow {
                    id: profile.id.into_bytes(),
                    name: profile.name.clone(),
                    current: profile.current,
                })
                .collect(),
        },
        reauthenticating: core.reauth.as_ref().map(|reauth| Reauthenticating {
            key: ui::account_key(&reauth.account),
            name: reauth.account.name.clone(),
            busy: core.pending_account_request.is_some(),
            error: reauth.error.clone(),
        }),
        discarding: core
            .pending_destructive_action
            .as_ref()
            .map(|action| match action {
                PendingDestructiveAction::ChooseOrganization => DiscardReason::SwitchOrganization,
                PendingDestructiveAction::Close => DiscardReason::Close,
            }),
        invitations: core.invitations.root_area(),
        members: core.members.root_area(core.account.id),
        agents: AgentsArea {
            listed: core.agents_open && core.pending_organization_request.is_none(),
            agents: core
                .agents
                .iter()
                .map(|agent| AgentRow {
                    account: agent.account.into_bytes(),
                    name: agent.name.clone(),
                })
                .collect(),
            busy: core.agents_open && core.pending_organization_request.is_some(),
            error: core
                .agents_open
                .then(|| core.organization_error.clone())
                .flatten(),
        },
        contacts: core.contacts.root_area(),
        share_links: core.share_links.root_area(),
        leaving: shown.leaving.as_ref().map(|leaving| Leaving {
            block: leaving.block.into_bytes(),
            name: leaving.name.clone(),
            within: leaving.within.clone(),
            busy: shown.leave_busy,
            refused: (!shown.leave_refused.is_empty()).then(|| shown.leave_refused.clone()),
        }),
        recovery: RecoveryArea {
            words: u32::try_from(shown.recovery.words.len()).unwrap_or(u32::MAX),
            // The host counts the words from 0; the root, as the person does, from 1.
            checked: shown
                .recovery
                .checked
                .iter()
                .map(|position| u32::try_from(position + 1).unwrap_or(u32::MAX))
                .collect(),
            replacing: shown.recovery.replacing,
            busy: shown.recovery.busy,
            error: shown.recovery.error.clone(),
        },
        unlock: UnlockArea {
            account: shown.unlock.account.clone(),
            pairing: shown.unlock.code.is_some(),
            error: shown.unlock.error.clone(),
        },
        pairing: shown
            .pairing
            .iter()
            .map(|request| PairingRequest {
                id: request.from,
                device: request.device.clone(),
                refused: (!request.refused.is_empty()).then(|| request.refused.clone()),
            })
            .collect(),
        failure: core.error.as_ref().map(|message| CoreFailure {
            message: message.clone(),
            unsaved: u32::try_from(shown.error.unsaved).unwrap_or(u32::MAX),
        }),
        organization: organization.map(|organization| organization.name.clone()),
        company: shown.status.company,
        administrator: shown.status.administrator,
        escrow_warning: shown.status.escrow_warning.clone(),
        hosting: shown
            .status
            .hosting
            .iter()
            .map(|(block, name)| HostedBlock {
                block: block.into_bytes(),
                name: name.clone(),
            })
            .collect(),
        default_server: be_host_base::platform::default_account_server().map(|server| {
            DefaultServer {
                name: server.name,
                url: server.api_url,
                gateway_required: server.gateway_required,
            }
        }),
        showing: core.root_session.map(Uuid::into_bytes),
    }
}

/// Why a block's changes were set aside, as the root words it.
fn set_aside(aside: be_block_client::Aside) -> SetAside {
    use be_block_protocol::ErrorCode;
    match aside {
        be_block_client::Aside::Refused(code) => SetAside::Refused(match code {
            ErrorCode::OverQuota => Refusal::OverQuota,
            ErrorCode::InvalidRequest => Refusal::Invalid,
            ErrorCode::PolicyRefused => Refusal::Policy,
            ErrorCode::PermissionDenied | ErrorCode::BlockNotFound => Refusal::NotPermitted,
            _ => Refusal::Unexplained,
        }),
        be_block_client::Aside::KeyOutdated => SetAside::KeyOutdated,
        be_block_client::Aside::Misplaced => SetAside::Misplaced,
    }
}
