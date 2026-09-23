use block::{Account, BlockAccess, BlockAccessEntry, WorkspaceRole};
use block_client::{BlockAccessRequest, BlockClient};
use uuid::Uuid;

use crate::editors::BlockLabel;

pub(crate) const GRANTABLE: [BlockAccess; 3] = [
    BlockAccess::Edit,
    BlockAccess::View,
    BlockAccess::KnowExists,
];

const MAX_SUGGESTIONS: usize = 6;

#[derive(Default)]
pub struct ShareDialog {
    open: Option<ShareState>,
}

struct ShareState {
    id: Uuid,
    label: BlockLabel,
    request: Option<BlockAccessRequest>,
    entries: Vec<BlockAccessEntry>,
    loaded: bool,
    error: Option<String>,
    query: String,
    pending: Vec<Account>,
    pending_access: BlockAccess,
}

#[derive(Clone, Debug)]
pub(crate) enum ShareCommand {
    SetQuery(String),
    Submit,
    Pick(Uuid),
    Unpick(Uuid),
    SetPendingAccess(BlockAccess),
    AddPending,
    SetAccess(Uuid, BlockAccess),
    Refresh,
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Suggestion {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) email: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Member {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) fixed: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) access: BlockAccess,
    pub(crate) removable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShareView {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) automatic: bool,
    pub(crate) error: Option<String>,
    pub(crate) loading: bool,
    pub(crate) refreshing: bool,
    pub(crate) query: String,
    pub(crate) suggestions: Option<Vec<Suggestion>>,
    pub(crate) pending: Vec<Suggestion>,
    pub(crate) pending_access: BlockAccess,
    pub(crate) members: Vec<Member>,
    pub(crate) nobody: bool,
}

impl ShareDialog {
    pub fn open(&mut self, client: &BlockClient, id: Uuid, label: BlockLabel) {
        self.open = Some(ShareState {
            id,
            label,
            request: Some(client.request_block_access(id)),
            entries: Vec::new(),
            loaded: false,
            error: None,
            query: String::new(),
            pending: Vec::new(),
            pending_access: BlockAccess::Edit,
        });
    }

    pub fn poll(&mut self, _client: &BlockClient) {
        if let Some(state) = &mut self.open {
            state.poll();
            if state.request.is_some() {
                crate::host::request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }

    pub fn command(&mut self, client: &BlockClient, command: ShareCommand) {
        let Some(state) = &mut self.open else {
            return;
        };
        let account_id = client.account_id();
        let mut grants = Vec::new();
        let mut reload = false;
        match command {
            ShareCommand::SetQuery(query) => state.query = query,
            ShareCommand::Submit => {
                if let Some(account) = state.candidates(account_id).first().cloned() {
                    state.pending.push(account);
                    state.query.clear();
                }
            }
            ShareCommand::Pick(id) => {
                if let Some(account) = state
                    .candidates(account_id)
                    .into_iter()
                    .find(|account| account.id == id)
                {
                    state.pending.push(account);
                    state.query.clear();
                }
            }
            ShareCommand::Unpick(id) => state.pending.retain(|account| account.id != id),
            ShareCommand::SetPendingAccess(access) => state.pending_access = access,
            ShareCommand::AddPending => {
                let access = state.pending_access;
                grants.extend(state.pending.drain(..).map(|account| (account.id, access)));
                state.query.clear();
            }
            ShareCommand::SetAccess(account, access) => grants.push((account, access)),
            ShareCommand::Refresh => reload = true,
            ShareCommand::Close => {
                self.open = None;
                return;
            }
        }
        for (account, access) in grants {
            client.set_block_access(state.id, account, access);
            reload = true;
        }
        if reload {
            state.error = None;
            state.request = Some(client.request_block_access(state.id));
        }
    }

    pub fn view(&self, client: &BlockClient) -> Option<ShareView> {
        let state = self.open.as_ref()?;
        let account_id = client.account_id();
        let members: Vec<Member> = state
            .entries
            .iter()
            .filter(|entry| has_access(entry, account_id))
            .map(|entry| member(entry, account_id))
            .collect();
        Some(ShareView {
            id: state.id,
            name: state.label.name.clone(),
            automatic: state.label.automatic,
            error: state.error.clone(),
            loading: !state.loaded && state.error.is_none(),
            refreshing: state.request.is_some(),
            query: state.query.clone(),
            suggestions: (!state.query.is_empty()).then(|| {
                state
                    .candidates(account_id)
                    .into_iter()
                    .take(MAX_SUGGESTIONS)
                    .map(|account| suggestion(&account))
                    .collect()
            }),
            pending: state.pending.iter().map(suggestion).collect(),
            pending_access: state.pending_access,
            nobody: state.loaded && members.is_empty(),
            members,
        })
    }
}

fn suggestion(account: &Account) -> Suggestion {
    Suggestion {
        id: account.id,
        name: account.display_name.clone(),
        email: account.email.clone(),
    }
}

fn member(entry: &BlockAccessEntry, account_id: Uuid) -> Member {
    let fixed = match entry.role {
        WorkspaceRole::Administrator => Some("Administrators can open every block"),
        WorkspaceRole::Editor if entry.account.id == account_id => Some("This is you"),
        WorkspaceRole::Editor => None,
    };
    Member {
        id: entry.account.id,
        name: entry.account.display_name.clone(),
        email: entry.account.email.clone(),
        fixed: fixed.map(str::to_owned),
        note: match fixed {
            Some(_) => None,
            None => (entry.granted != Some(entry.effective))
                .then(|| format!("Inherited: {}", entry.effective.label())),
        },
        access: match fixed {
            Some(_) => entry.effective,
            None => entry.granted.unwrap_or(BlockAccess::None),
        },
        removable: fixed.is_none() && entry.granted != Some(BlockAccess::None),
    }
}

impl ShareState {
    fn poll(&mut self) {
        let Some(result) = self.request.as_mut().and_then(BlockAccessRequest::poll) else {
            return;
        };
        self.request = None;
        match result {
            Ok(entries) => {
                self.entries = entries;
                self.loaded = true;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn candidates(&self, account_id: Uuid) -> Vec<Account> {
        let query = self.query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|entry| !has_access(entry, account_id))
            .filter(|entry| {
                !self
                    .pending
                    .iter()
                    .any(|pending| pending.id == entry.account.id)
            })
            .filter(|entry| {
                query.is_empty()
                    || entry.account.display_name.to_lowercase().contains(&query)
                    || entry.account.email.to_lowercase().contains(&query)
            })
            .map(|entry| entry.account.clone())
            .collect()
    }
}

fn has_access(entry: &BlockAccessEntry, account_id: Uuid) -> bool {
    matches!(entry.role, WorkspaceRole::Administrator)
        || entry.account.id == account_id
        || entry.granted.is_some()
        || entry.effective > BlockAccess::None
}
