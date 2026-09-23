mod app_state;
mod be;
mod block_label;
mod block_picker;
mod debug;
mod editors;
mod files;
mod host;
mod panic_guard;
mod performance;
mod platform;
mod plugin_host;
mod share;
mod slide_templates;
mod surfaces;
mod ui;

use std::{collections::HashMap, error::Error, sync::Arc, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
use std::{io, path::PathBuf};

use app_state::{AppStateStore, SavedAccount, ServerLocation};
use beui::Document;
use block::{
    Block, BlockAccess, BlockParent, ManagementErrorCode, Workspace, WorkspaceInvitation,
    WorkspaceRole,
};
use block_client::root_settings::{RootSetting, RootSettings};
use block_client::{
    BlockClient, BlockHandle, DynamicArtifactDescriptor, ManagementClient, ManagementClientError,
    Session,
    blocks::{
        file_tree::FileTree, ui_settings::UiSettings, workspace_index::BlockEntry,
        workspace_ui::WorkspaceUi,
    },
    properties::MAX_NAME_BYTES,
};
use block_plugin_api::{AccessLevel, ArtifactAction, BlockCommand, BlockLocation};
use editors::{
    ArtifactSession, ArtifactStatus, BlockLabel, EditorAccess, EditorAction, EditorRegistry,
    PluginEditor, SidebarDragSource, direct_editor_tab_ui,
};
use share::ShareDialog;
use surfaces::SurfaceId;
use ui::{AccountForm, AppView, AppViewStore, ErrorAction, UiCommand};
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
const APP_ID: &str = "Block";

pub(crate) const COMMIT: &str = env!("BLOCK_APP_COMMIT");

fn run_options() -> beui::RunOptions {
    let mut options = beui::RunOptions::new("Block");
    #[cfg(not(target_arch = "wasm32"))]
    {
        options.app_id = Some(APP_ID.to_owned());
    }
    options.size = beui::vec2(1100.0, 720.0);
    options
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_dir() -> Option<PathBuf> {
    directories_next::ProjectDirs::from("", "", APP_ID).map(|dirs| dirs.data_dir().to_path_buf())
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run() -> Result<(), Box<dyn Error>> {
    panic_guard::install();
    let app = BlockApp::new(None).map_err(|error| error.to_string())?;
    beui::run_with(run_options(), Shell::new(app))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_web(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    panic_guard::install();
    editors::plugin::discovery::load().await;
    let app =
        BlockApp::new().map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    beui::run_web(&canvas_id, run_options(), Shell::new(app))
        .await
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    editors::plugin::discovery::load(&app);
    panic_guard::install();
    let storage_root = app.internal_data_path();
    let mut options = run_options();
    options.android_app = Some(app);
    let exit_code = match BlockApp::new(storage_root)
        .map_err(|error| error.to_string())
        .and_then(|block_app| {
            beui::run_with(options, Shell::new(block_app)).map_err(|error| error.to_string())
        }) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Block stopped: {error}");
            1
        }
    };

    std::process::exit(exit_code);
}

struct Shell {
    document: Document,
    view: AppViewStore,
    app: BlockApp,
}

impl Shell {
    fn new(app: BlockApp) -> Self {
        let mut view = None;
        let document = beui::reactive::build(|| {
            surfaces::create_handles();
            let store = AppViewStore::new(AppView::default());
            view = Some(store.clone());
            ui::root(store)
        });
        Self {
            document,
            view: view.expect("the view store is created while the document is built"),
            app,
        }
    }
}

impl beui::App for Shell {
    fn setup(&mut self, setup: &beui::Setup) {
        host::install_waker(setup.waker.clone());
        plugin_host::install(setup);
        #[cfg(all(
            feature = "web-view",
            not(target_os = "android"),
            not(target_arch = "wasm32")
        ))]
        plugin_host::install_web_view(setup.window.clone());
    }

    fn update(&mut self, context: &beui::Context, rect: beui::Rect) {
        host::begin(context, &self.document);
        self.app.frame(context);
        if let Some(open) = self.app.inspector_requested.take()
            && open
        {
            self.document.open_inspector();
        }
        let view = self.app.view();
        let store = self.view.clone();
        beui::reactive::with_reactive_scope(&mut self.document, move || {
            store.set(view);
            surfaces::commit();
        });
        host::filter_document_input(context);
        self.document.show(context, rect);
        surfaces::read_placements();
        let commands = ui::take_commands();
        if !commands.is_empty() {
            for command in commands {
                self.app.command(context, command);
            }
            context.request_repaint();
        }
        host::end(context);
    }

    fn clear_color(&self) -> beui::Color32 {
        self.document.theme().background
    }

    fn close_requested(&mut self) -> bool {
        self.app.close_requested()
    }

    fn exiting(&mut self) {
        block_client::shut_down_clients();
    }
}

struct BlockApp {
    app_state: AppStateStore,

    client_id: Uuid,
    local_server_url: String,
    accounts: Vec<Account>,
    signed_in: bool,
    add_account_open: bool,
    add_account_generation: u64,
    pending_account_request: Option<PendingAccountRequest>,
    account_error: Option<String>,
    workspace: Option<Workspace>,
    workspaces: Vec<Workspace>,
    invitations: Vec<WorkspaceInvitation>,
    workspaces_loaded: bool,
    workspaces_load_failed: bool,
    pending_workspace_request:
        Option<platform::RequestResult<Result<WorkspaceResult, WorkspaceRequestError>>>,
    workspace_created: u64,
    workspace_error: Option<String>,

    reauth: Option<ReauthState>,
    invite_open: bool,
    invite_sent: u64,
    scheduled_workspace_list: bool,
    server_url: String,
    account: Account,
    client: Arc<BlockClient>,
    root_settings: RootSettings,
    file_tree: RootSetting<FileTree>,
    workspace_ui: RootSetting<WorkspaceUi>,
    shell: Option<Uuid>,
    ui_settings: Option<Uuid>,
    block_types: HashMap<Uuid, Uuid>,
    registry: EditorRegistry,
    editors: HashMap<Uuid, PluginEditor>,

    editor_access: HashMap<Uuid, BlockAccess>,

    watched_artifacts: Vec<Uuid>,
    dynamic_artifact_sessions: HashMap<Uuid, Box<dyn ArtifactSession>>,
    dynamic_artifact_errors: HashMap<Uuid, String>,

    dynamic_artifact_settings: HashMap<Uuid, Vec<u8>>,

    dynamic_artifact_settings_open: Option<Uuid>,

    dynamic_artifact_unlink: Option<Uuid>,

    pending_transfers: Vec<PendingTransfer>,
    pending_copies: Vec<PendingCopy>,
    rename: Option<RenameState>,
    share: ShareDialog,
    about_open: bool,
    pending_destructive_action: Option<PendingDestructiveAction>,
    scheduled_account_switch: Option<Account>,
    allow_close: bool,
    #[cfg(not(target_arch = "wasm32"))]
    data_dir: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    embedded_server: Option<platform::EmbeddedServer>,
    error: Option<String>,
    pending_error_action: Option<ErrorAction>,
    inspector_requested: Option<bool>,
}

type Account = SavedAccount;

struct PendingAccountRequest {
    receiver: platform::RequestResult<Result<Session, String>>,
    server: ServerLocation,
    url: String,
}

enum WorkspaceOperation {
    Load,
    Create(String),
    Respond(Uuid, bool),
    Invite(Uuid, String, WorkspaceRole),
}

enum WorkspaceResult {
    Loaded(Vec<Workspace>, Vec<WorkspaceInvitation>),
    Created(Workspace),
    Responded,
    Invited,
}

struct WorkspaceRequestError {
    message: String,
    invalid_token: bool,
}

impl From<ManagementClientError> for WorkspaceRequestError {
    fn from(error: ManagementClientError) -> Self {
        let invalid_token = matches!(
            error,
            ManagementClientError::Server {
                code: ManagementErrorCode::InvalidToken,
                ..
            }
        );
        Self {
            message: error.to_string(),
            invalid_token,
        }
    }
}

struct ReauthState {
    account: Account,
    pending: Option<platform::RequestResult<Result<Session, String>>>,
    error: Option<String>,
}

#[derive(Clone)]
enum PendingDestructiveAction {
    Switch(Account),
    ChooseWorkspace,
    Close,
}

#[derive(Clone)]
struct PendingTransfer {
    child: Uuid,
    source: Option<SidebarDragSource>,
    destination: Option<Uuid>,
    parent_after: Option<BlockParent>,
    stage: TransferStage,
}

#[derive(Clone, Copy)]
enum TransferStage {
    DeleteSource,
    AddDestination,
}

#[derive(Clone)]
struct PendingCopy {
    source: Uuid,
    container: Uuid,
    stage: CopyStage,
}

#[derive(Clone, Copy)]
enum CopyStage {
    Duplicate,
    Replace { copy_id: Uuid, block_type: Uuid },
}

struct RenameState {
    id: Uuid,
    name: String,
}

impl BlockApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(storage_root: Option<PathBuf>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = storage_root
            .or_else(storage_dir)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?;
        std::fs::create_dir_all(&data_dir)?;
        plugin_host::cache_in(data_dir.join("plugin-cache"));
        let embedded_server = platform::start_embedded_server(data_dir.join("server"))?;
        let url = embedded_server.url.clone();
        let app_state = AppStateStore::open(data_dir.join("app.sqlite3"))?;
        let mut app = Self::with_state(app_state, url)?;
        app.data_dir = data_dir;
        app.embedded_server = Some(embedded_server);
        Ok(app)
    }

    #[cfg(target_arch = "wasm32")]
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let app_state = AppStateStore::open()?;
        Self::with_state(app_state, String::new())
    }

    fn with_state(
        app_state: AppStateStore,
        url: String,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let client_id = app_state.client_id()?;
        let accounts = app_state.accounts()?;
        let active = app_state.active_account()?;
        let account = active
            .and_then(|(server, id)| {
                accounts
                    .iter()
                    .find(|account| account.server.key() == server && account.id == id)
            })
            .cloned();
        let signed_in = account.is_some();
        let account = account.unwrap_or_else(|| Account {
            server: ServerLocation::Local,
            id: Uuid::nil(),
            email: String::new(),
            name: String::new(),
            token: String::new(),
            last_workspace_id: None,
        });
        let server_url = match &account.server {
            ServerLocation::Local => url.clone(),
            ServerLocation::Remote(remote) => remote.clone(),
        };
        let client = Arc::new(BlockClient::new(account.id, Uuid::nil()));
        let root_settings = RootSettings::new(&client);
        let file_tree = RootSetting::new(&client);
        let workspace_ui = RootSetting::new(&client);
        Ok(Self {
            app_state,
            client_id,
            local_server_url: url,
            accounts,
            signed_in,
            add_account_open: false,
            add_account_generation: 0,
            pending_account_request: None,
            account_error: None,
            workspace: None,
            workspaces: Vec::new(),
            invitations: Vec::new(),
            workspaces_loaded: false,
            workspaces_load_failed: false,
            pending_workspace_request: None,
            workspace_created: 0,
            workspace_error: None,
            reauth: None,
            invite_open: false,
            invite_sent: 0,
            scheduled_workspace_list: false,
            server_url,
            account,
            client,
            root_settings,
            file_tree,
            workspace_ui,
            shell: None,
            ui_settings: None,
            block_types: HashMap::new(),
            registry: EditorRegistry::new(),
            editors: HashMap::new(),
            editor_access: HashMap::new(),
            watched_artifacts: Vec::new(),
            dynamic_artifact_sessions: HashMap::new(),
            dynamic_artifact_errors: HashMap::new(),
            dynamic_artifact_settings: HashMap::new(),
            dynamic_artifact_settings_open: None,
            dynamic_artifact_unlink: None,
            pending_transfers: Vec::new(),
            pending_copies: Vec::new(),
            rename: None,
            share: ShareDialog::default(),
            about_open: false,
            pending_destructive_action: None,
            scheduled_account_switch: None,
            allow_close: false,
            #[cfg(not(target_arch = "wasm32"))]
            data_dir: PathBuf::new(),
            #[cfg(not(target_arch = "wasm32"))]
            embedded_server: None,
            error: None,
            pending_error_action: None,
            inspector_requested: None,
        })
    }

    fn account_by_key(&self, key: &str) -> Option<Account> {
        self.accounts
            .iter()
            .find(|account| ui::account_key(account) == key)
            .cloned()
    }

    fn begin_account_request(&mut self, form: AccountForm) {
        let remote = form.remote || !platform::HAS_EMBEDDED_SERVER;
        let requested_url = if remote {
            form.remote_url.clone()
        } else {
            self.local_server_url.clone()
        };
        let client = match ManagementClient::new(requested_url) {
            Ok(client) => client,
            Err(error) => {
                self.account_error = Some(error.to_string());
                return;
            }
        };
        let server = if remote {
            ServerLocation::Remote(client.url().to_owned())
        } else {
            ServerLocation::Local
        };
        let url = client.url().to_owned();
        let AccountForm {
            register,
            email,
            display_name,
            password,
            ..
        } = form;
        let receiver = platform::spawn_request(async move {
            if register {
                client.register(email, display_name, password).await
            } else {
                client.login(email, password).await
            }
            .map_err(|error| error.to_string())
        });
        self.account_error = None;
        self.pending_account_request = Some(PendingAccountRequest {
            receiver,
            server,
            url,
        });
    }

    fn poll_account_request(&mut self) {
        let result = self
            .pending_account_request
            .as_ref()
            .and_then(|pending| pending.receiver.try_recv().ok());
        let Some(result) = result else {
            if self.pending_account_request.is_some() {
                host::request_repaint_after(Duration::from_millis(100));
            }
            return;
        };
        let pending = self.pending_account_request.take().unwrap();
        match result {
            Ok(session) => {
                let saved = SavedAccount {
                    server: pending.server,
                    id: session.account.id,
                    email: session.account.email,
                    name: session.account.display_name,
                    token: session.token,
                    last_workspace_id: None,
                };
                if let Err(error) = self.app_state.save_account(&saved) {
                    self.account_error = Some(error.to_string());
                    return;
                }
                self.accounts = self.app_state.accounts().unwrap_or_else(|error| {
                    self.account_error = Some(error.to_string());
                    Vec::new()
                });
                self.server_url = pending.url;
                self.add_account_open = false;
                self.switch_account(saved);
            }
            Err(error) => self.account_error = Some(error),
        }
    }

    fn open_account(&mut self, mut account: Account, choose_workspace: bool) {
        if choose_workspace {
            account.last_workspace_id = None;
            if let Err(error) = self.app_state.set_last_workspace(&account, None) {
                self.account_error = Some(error.to_string());
            }
        }
        self.switch_account(account);
    }

    fn log_out_account(&mut self, account: &Account) {
        let url = match &account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        let token = account.token.clone();
        let _ = platform::spawn_request(async move {
            if let Ok(client) = ManagementClient::new(url) {
                let _ = client.logout(token).await;
            }
        });

        if let Err(error) = self.app_state.remove_account(account) {
            self.account_error = Some(error.to_string());
            return;
        }
        self.accounts
            .retain(|saved| saved.server != account.server || saved.id != account.id);
        if self.account.server == account.server && self.account.id == account.id {
            self.signed_in = false;
        }
    }

    fn begin_workspace_request(&mut self, operation: WorkspaceOperation) {
        if self.pending_workspace_request.is_some() {
            return;
        }
        let client = match ManagementClient::new(self.server_url.clone()) {
            Ok(client) => client,
            Err(error) => {
                self.workspace_error = Some(error.to_string());
                if matches!(operation, WorkspaceOperation::Load) {
                    self.workspaces_load_failed = true;
                }
                return;
            }
        };
        let token = self.account.token.clone();
        let receiver = platform::spawn_request(async move {
            match operation {
                WorkspaceOperation::Load => {
                    let workspaces = client
                        .list_workspaces(&token)
                        .await
                        .map_err(WorkspaceRequestError::from)?;
                    let invitations = client
                        .list_invitations(&token)
                        .await
                        .map_err(WorkspaceRequestError::from)?;
                    Ok(WorkspaceResult::Loaded(workspaces, invitations))
                }
                WorkspaceOperation::Create(name) => client
                    .create_workspace(&token, name)
                    .await
                    .map(WorkspaceResult::Created)
                    .map_err(WorkspaceRequestError::from),
                WorkspaceOperation::Respond(invitation_id, accept) => client
                    .respond_invitation(&token, invitation_id, accept)
                    .await
                    .map(|()| WorkspaceResult::Responded)
                    .map_err(WorkspaceRequestError::from),
                WorkspaceOperation::Invite(workspace_id, email, role) => client
                    .invite(&token, workspace_id, email, role)
                    .await
                    .map(|_| WorkspaceResult::Invited)
                    .map_err(WorkspaceRequestError::from),
            }
        });
        self.workspace_error = None;
        self.pending_workspace_request = Some(receiver);
    }

    fn poll_workspace_request(&mut self) {
        let result = self
            .pending_workspace_request
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(result) = result else {
            if self.pending_workspace_request.is_some() {
                host::request_repaint_after(Duration::from_millis(100));
            }
            return;
        };
        self.pending_workspace_request = None;
        match result {
            Ok(WorkspaceResult::Loaded(workspaces, invitations)) => {
                self.workspaces = workspaces;
                self.invitations = invitations;
                self.workspaces_loaded = true;
                self.workspaces_load_failed = false;
                if let Some(last_workspace_id) = self.account.last_workspace_id
                    && let Some(workspace) = self
                        .workspaces
                        .iter()
                        .find(|workspace| workspace.id == last_workspace_id)
                        .cloned()
                {
                    self.open_workspace(workspace);
                }
            }
            Ok(WorkspaceResult::Created(workspace)) => {
                self.workspaces.push(workspace.clone());
                self.workspace_created += 1;
                self.open_workspace(workspace);
            }
            Ok(WorkspaceResult::Responded) => {
                self.workspaces_loaded = false;
                self.workspaces_load_failed = false;
                self.begin_workspace_request(WorkspaceOperation::Load);
            }
            Ok(WorkspaceResult::Invited) => {
                self.invite_sent += 1;
                self.invite_open = false;
            }
            Err(error) if error.invalid_token => self.begin_reauth(),
            Err(error) => {
                if !self.workspaces_loaded {
                    self.workspaces_load_failed = true;
                }
                self.workspace_error = Some(error.message);
            }
        }
    }

    fn begin_reauth(&mut self) {
        if self.reauth.is_some() {
            return;
        }
        self.reauth = Some(ReauthState {
            account: self.account.clone(),
            pending: None,
            error: None,
        });
    }

    fn begin_reauth_request(&mut self, password: String) {
        let Some(reauth) = &self.reauth else {
            return;
        };
        if reauth.pending.is_some() || password.is_empty() {
            return;
        }
        let url = match &reauth.account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        let email = reauth.account.email.clone();
        let client = match ManagementClient::new(url) {
            Ok(client) => client,
            Err(error) => {
                self.reauth.as_mut().unwrap().error = Some(error.to_string());
                return;
            }
        };
        let receiver = platform::spawn_request(async move {
            client
                .login(email, password)
                .await
                .map_err(|error| error.to_string())
        });
        let reauth = self.reauth.as_mut().unwrap();
        reauth.error = None;
        reauth.pending = Some(receiver);
    }

    fn poll_reauth_request(&mut self) {
        let result = self
            .reauth
            .as_ref()
            .and_then(|reauth| reauth.pending.as_ref())
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(result) = result else {
            if self
                .reauth
                .as_ref()
                .is_some_and(|reauth| reauth.pending.is_some())
            {
                host::request_repaint_after(Duration::from_millis(100));
            }
            return;
        };
        let Some(mut reauth) = self.reauth.take() else {
            return;
        };
        reauth.pending = None;
        match result {
            Ok(session) => {
                let mut updated = reauth.account.clone();
                updated.id = session.account.id;
                updated.email = session.account.email;
                updated.name = session.account.display_name;
                updated.token = session.token;
                if let Err(error) = self.app_state.save_account(&updated) {
                    reauth.error = Some(error.to_string());
                    self.reauth = Some(reauth);
                    return;
                }
                if let Some(saved) = self
                    .accounts
                    .iter_mut()
                    .find(|saved| saved.server == updated.server && saved.id == updated.id)
                {
                    *saved = updated.clone();
                }
                if self.account.server == updated.server && self.account.id == updated.id {
                    self.account.token = updated.token;
                }
                self.workspace_error = None;
                self.workspaces_loaded = false;
                self.workspaces_load_failed = false;
                self.begin_workspace_request(WorkspaceOperation::Load);
            }
            Err(error) => {
                reauth.error = Some(error);
                self.reauth = Some(reauth);
            }
        }
    }

    fn close_reauth(&mut self) {
        self.reauth = None;
        self.signed_in = false;
        if let Err(error) = self.app_state.clear_active_account() {
            self.account_error = Some(error.to_string());
        }
    }

    fn open_workspace(&mut self, workspace: Workspace) {
        be::stop();
        let client = Arc::new(BlockClient::new(self.account.id, workspace.id));
        client.connect(self.server_url.clone(), self.account.token.clone());
        self.block_types.clear();
        self.registry = EditorRegistry::new();
        self.editors.clear();
        self.watched_artifacts.clear();
        self.dynamic_artifact_sessions.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.dynamic_artifact_unlink = None;
        self.share = ShareDialog::default();
        self.root_settings = RootSettings::new(&client);
        self.file_tree = RootSetting::new(&client);
        self.workspace_ui = RootSetting::new(&client);
        self.shell = None;
        self.ui_settings = None;
        self.client = client;
        self.workspace = Some(workspace.clone());
        self.account.last_workspace_id = Some(workspace.id);
        if let Some(saved) = self
            .accounts
            .iter_mut()
            .find(|saved| saved.server == self.account.server && saved.id == self.account.id)
        {
            saved.last_workspace_id = Some(workspace.id);
        }
        if let Err(error) = self
            .app_state
            .set_last_workspace(&self.account, Some(workspace.id))
        {
            self.workspace_error = Some(error.to_string());
        }
    }

    fn load_workspaces_if_needed(&mut self) {
        if !self.workspaces_loaded
            && !self.workspaces_load_failed
            && self.pending_workspace_request.is_none()
            && self.reauth.is_none()
        {
            self.begin_workspace_request(WorkspaceOperation::Load);
        }
        self.poll_workspace_request();
    }

    fn request_account_switch(&mut self, account: Account) {
        if account == self.account {
            return;
        }
        if self.client.network_debug_snapshot().changes_saved {
            self.scheduled_account_switch = Some(account);
        } else {
            self.pending_destructive_action = Some(PendingDestructiveAction::Switch(account));
        }
    }

    fn switch_account(&mut self, account: Account) {
        let server_url = match &account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        let client = Arc::new(BlockClient::new(account.id, Uuid::nil()));
        self.block_types.clear();
        self.registry = EditorRegistry::new();
        self.editors.clear();
        self.watched_artifacts.clear();
        self.dynamic_artifact_sessions.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.dynamic_artifact_unlink = None;
        self.pending_transfers.clear();
        self.rename = None;
        self.share = ShareDialog::default();
        debug::close_client_windows();
        self.about_open = false;
        self.pending_destructive_action = None;
        self.scheduled_account_switch = None;
        self.allow_close = false;
        self.workspace = None;
        self.workspaces.clear();
        self.invitations.clear();
        self.workspaces_loaded = false;
        self.workspaces_load_failed = false;
        self.pending_workspace_request = None;
        self.workspace_error = None;
        self.reauth = None;
        self.invite_open = false;
        self.root_settings = RootSettings::new(&client);
        self.file_tree = RootSetting::new(&client);
        self.workspace_ui = RootSetting::new(&client);
        self.shell = None;
        self.ui_settings = None;
        self.client = client;
        self.account = account;
        self.server_url = server_url;
        self.signed_in = true;
        if let Err(error) = self.app_state.set_active_account(&self.account) {
            self.account_error = Some(error.to_string());
        }
        host::clear_focus();
    }

    fn close_requested(&mut self) -> bool {
        be::flush();
        if self.allow_close || self.client.network_debug_snapshot().changes_saved {
            return true;
        }
        self.pending_destructive_action = Some(PendingDestructiveAction::Close);
        host::request_repaint();
        false
    }

    fn discard(&mut self, context: &beui::Context) {
        let Some(action) = self.pending_destructive_action.take() else {
            return;
        };
        match action {
            PendingDestructiveAction::Switch(account) => {
                self.scheduled_account_switch = Some(account);
            }
            PendingDestructiveAction::ChooseWorkspace => {
                self.scheduled_workspace_list = true;
            }
            PendingDestructiveAction::Close => {
                self.allow_close = true;
                context.close_window();
            }
        }
    }

    fn block_type_of(&self, id: Uuid) -> Option<Uuid> {
        self.block_types.get(&id).copied().or_else(|| {
            self.client
                .cached_block(id)
                .map(|cached| cached.block_type)
                .or_else(|| self.editors.get(&id).map(|editor| editor.block_type()))
        })
    }

    fn ensure_editor(&mut self, id: Uuid) -> bool {
        if self.editors.contains_key(&id) {
            return true;
        }
        let Some(block_type) = self.block_type_of(id) else {
            return false;
        };
        self.block_types.insert(id, block_type);
        self.editors
            .insert(id, self.registry.open(&self.client, id, block_type));
        true
    }

    fn queue_placement(&mut self, child: Uuid, block_type: Uuid, parent: Uuid, linked: bool) {
        if child == parent
            || self
                .pending_transfers
                .iter()
                .any(|pending| pending.child == child && pending.destination == Some(parent))
        {
            return;
        }
        self.block_types.insert(child, block_type);
        if !self.ensure_editor(parent) {
            return;
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: None,
            destination: Some(parent),
            parent_after: (!linked).then_some(BlockParent::Uuid(parent)),
            stage: TransferStage::AddDestination,
        });
    }

    fn set_block_parent(&mut self, id: Uuid, parent: BlockParent) {
        if let Some(editor) = self.editors.get(&id) {
            editor.set_parent(parent);
        } else {
            self.client.set_block_parent(id, parent);
        }
    }

    fn queue_move(
        &mut self,
        child: Uuid,
        block_type: Uuid,
        source: SidebarDragSource,
        destination: Uuid,
        is_reference: bool,
    ) {
        if self
            .pending_transfers
            .iter()
            .any(|pending| pending.child == child && pending.destination == Some(destination))
        {
            return;
        }
        self.block_types.insert(child, block_type);
        let source_ready = match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(source) => self.ensure_editor(source),
        };
        if !source_ready || !self.ensure_editor(destination) {
            return;
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: Some(source),
            destination: Some(destination),
            parent_after: (!is_reference).then_some(BlockParent::Uuid(destination)),
            stage: TransferStage::DeleteSource,
        });
    }

    fn queue_delete(
        &mut self,
        child: Uuid,
        block_type: Uuid,
        source: SidebarDragSource,
        is_reference: bool,
    ) {
        if self.pending_transfers.iter().any(|pending| {
            pending.child == child
                && pending.source == Some(source)
                && pending.destination.is_none()
        }) {
            return;
        }
        self.block_types.insert(child, block_type);
        let ready = match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(source) => self.ensure_editor(source),
        };
        if !ready {
            return;
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: Some(source),
            destination: None,
            parent_after: (!is_reference && source != SidebarDragSource::Root)
                .then_some(BlockParent::Orphaned),
            stage: TransferStage::DeleteSource,
        });
    }

    fn process_pending_transfers(&mut self) {
        let pending = std::mem::take(&mut self.pending_transfers);
        for mut transfer in pending {
            if matches!(transfer.stage, TransferStage::DeleteSource) {
                let ready = match transfer.source {
                    None | Some(SidebarDragSource::Orphaned) => Some(true),
                    Some(SidebarDragSource::Root) => {
                        self.client
                            .set_block_parent(transfer.child, BlockParent::Orphaned);
                        Some(true)
                    }
                    Some(SidebarDragSource::Block(source)) => self
                        .editors
                        .get(&source)
                        .and_then(|editor| editor.delete_child(BlockEntry { id: transfer.child })),
                };
                if ready != Some(true) {
                    self.pending_transfers.push(transfer);
                    continue;
                }
                transfer.stage = TransferStage::AddDestination;
            }

            let ready = transfer.destination.map_or(Some(true), |destination| {
                self.editors
                    .get(&destination)
                    .and_then(|editor| editor.add_child(BlockEntry { id: transfer.child }))
            });
            if ready != Some(true) {
                self.pending_transfers.push(transfer);
                continue;
            }

            if let Some(parent) = transfer.parent_after {
                self.set_block_parent(transfer.child, parent);
            }
        }
    }

    fn queue_copy(&mut self, source: Uuid, container: Uuid) {
        if self
            .pending_copies
            .iter()
            .any(|pending| pending.source == source && pending.container == container)
        {
            return;
        }
        self.ensure_editor(source);
        self.ensure_editor(container);
        self.pending_copies.push(PendingCopy {
            source,
            container,
            stage: CopyStage::Duplicate,
        });
    }

    fn process_pending_copies(&mut self) {
        let pending = std::mem::take(&mut self.pending_copies);
        for mut copy in pending {
            if let CopyStage::Duplicate = copy.stage {
                if !self.ensure_editor(copy.source) {
                    self.pending_copies.push(copy);
                    continue;
                }
                let Some((copy_id, block_type)) =
                    self.editors.get(&copy.source).and_then(|editor| {
                        editor
                            .block()
                            .duplicate(&self.client)
                            .map(|copy_id| (copy_id, editor.block_type()))
                    })
                else {
                    self.pending_copies.push(copy);
                    continue;
                };
                be::duplicate(copy.source, copy_id, block_type);
                copy.stage = CopyStage::Replace {
                    copy_id,
                    block_type,
                };
            }
            let CopyStage::Replace {
                copy_id,
                block_type,
            } = copy.stage
            else {
                unreachable!("copy stage was just set to Replace")
            };

            if !self.ensure_editor(copy.container) {
                self.pending_copies.push(copy);
                continue;
            }
            let replaced = self
                .editors
                .get(&copy.container)
                .and_then(|editor| editor.replace_child(copy.source, BlockEntry { id: copy_id }));
            if replaced != Some(true) {
                self.pending_copies.push(copy);
                continue;
            }

            self.set_block_parent(copy_id, BlockParent::Uuid(copy.container));
            self.show_in_shell(copy_id, block_type, Some(copy.container), Some(copy.source));
        }
    }

    fn editor_access_ceiling(&self, id: Uuid) -> BlockAccess {
        editors::editor_access_ceiling(&self.client, id)
    }

    fn editor_access(&self, id: Uuid) -> BlockAccess {
        let ceiling = self.editor_access_ceiling(id);
        self.editor_access
            .get(&id)
            .map_or(ceiling, |chosen| (*chosen).min(ceiling))
    }

    fn forget_dynamic_artifact_dialogs(&mut self, id: Uuid) {
        self.dynamic_artifact_settings.remove(&id);
        if self.dynamic_artifact_settings_open == Some(id) {
            self.dynamic_artifact_settings_open = None;
        }
        if self.dynamic_artifact_unlink == Some(id) {
            self.dynamic_artifact_unlink = None;
        }
    }

    fn ensure_shell(&mut self) -> Option<Uuid> {
        self.file_tree.ensure(&self.client, self.client_id);
        let id = self
            .workspace_ui
            .ensure(&self.client, self.client_id)
            .map(BlockHandle::id)?;
        self.block_types.insert(id, WorkspaceUi::TYPE_ID);
        if !self.editors.contains_key(&id) {
            let editor = self.registry.open(&self.client, id, WorkspaceUi::TYPE_ID);
            self.editors.insert(id, editor);
        }
        self.shell = Some(id);
        Some(id)
    }

    fn show_in_shell(&mut self, id: Uuid, block_type: Uuid, via: Option<Uuid>, from: Option<Uuid>) {
        self.block_types.insert(id, block_type);
        let Some(shell) = self.shell.and_then(|shell| self.editors.get(&shell)) else {
            return;
        };
        shell.show_block(id, block_type, via, from);
    }
    fn close_editor(&mut self, id: Uuid) {
        if self.shell == Some(id) {
            return;
        }
        self.editor_access.remove(&id);
        self.dynamic_artifact_sessions.remove(&id);
        self.dynamic_artifact_errors.remove(&id);
        self.forget_dynamic_artifact_dialogs(id);
        self.watched_artifacts.retain(|watched| *watched != id);
        if let Some(mut editor) = self.editors.remove(&id) {
            editor.tab_closed();
        }
    }

    fn watch_artifacts(&mut self, blocks: Vec<Uuid>) {
        for id in std::mem::replace(&mut self.watched_artifacts, blocks) {
            if !self.watched_artifacts.contains(&id) {
                self.dynamic_artifact_sessions.remove(&id);
                self.dynamic_artifact_errors.remove(&id);
                self.forget_dynamic_artifact_dialogs(id);
            }
        }
    }

    fn act_on_artifact(&mut self, id: Uuid, action: ArtifactAction) {
        match action {
            ArtifactAction::Regenerate => {
                let data = self
                    .client
                    .dynamic_artifact(id)
                    .map(|descriptor| descriptor.data);
                if let (Some(data), Some(session)) =
                    (data, self.dynamic_artifact_sessions.get_mut(&id))
                {
                    session.regenerate(&self.client, &data);
                    self.dynamic_artifact_errors.remove(&id);
                }
            }
            ArtifactAction::Settings => self.dynamic_artifact_settings_open = Some(id),
            ArtifactAction::Unlink => self.dynamic_artifact_unlink = Some(id),
        }
    }

    fn show_shell(&mut self) {
        let Some(shell) = self.ensure_shell() else {
            return;
        };
        for editor in self.editors.values_mut() {
            editor.set_tab_active(false);
        }
        let Some(mut editor) = self.editors.remove(&shell) else {
            return;
        };
        editor.set_tab_active(true);
        let access = self.editor_access(shell);
        let action = {
            let mut editors = EditorAccess::new(
                shell,
                access,
                &self.client,
                self.client_id,
                &self.registry,
                &mut self.editors,
                &self.editor_access,
            );
            surfaces::with(SurfaceId::Main, |ui| {
                direct_editor_tab_ui(&mut editor, ui, &mut editors)
            })
            .flatten()
        };
        let focus = editor.take_focus_report();
        let watch = editor.take_artifact_watch();
        self.editors.insert(shell, editor);
        if let Some(focus) = focus {
            plugin_host::set_focus(focus.block, focus.via);
        }
        if let Some(watch) = watch {
            self.watch_artifacts(watch);
        }
        for (id, editor) in self.editors.iter_mut() {
            if *id != shell {
                editor.finish_frame();
            }
        }
        if let Some(action) = action {
            self.handle_editor_action(action);
        }
    }

    fn poll_artifacts(&mut self) {
        let watched = self.watched_artifacts.clone();
        let mut states = Vec::new();
        for id in watched {
            let Some(descriptor) = self.client.dynamic_artifact(id) else {
                self.dynamic_artifact_sessions.remove(&id);
                self.forget_dynamic_artifact_dialogs(id);
                continue;
            };
            let mut session = self.dynamic_artifact_sessions.remove(&id);
            let mut unsupported = None;
            if session.is_none() {
                let block_type = self.block_type_of(id).unwrap_or_default();
                match self.registry.artifact_session(
                    descriptor.source_type,
                    id,
                    block_type,
                    self.client_id,
                ) {
                    Ok(started) => session = Some(started),
                    Err(error) => unsupported = Some(error),
                }
            }
            let status = session
                .as_mut()
                .map(|session| session.poll(&self.registry, &self.client, &descriptor.data));
            if let Some(outcome) = session.as_mut().and_then(|session| session.take_outcome()) {
                match outcome {
                    Ok(()) => {
                        self.dynamic_artifact_errors.remove(&id);
                    }
                    Err(error) => {
                        self.dynamic_artifact_errors.insert(id, error);
                    }
                }
            }
            let mut state = block_plugin_api::ArtifactState {
                block_id: id.into_bytes(),
                source_type: descriptor.source_type.into_bytes(),
                source: None,
                summary: String::new(),
                error: self.dynamic_artifact_errors.get(&id).cloned(),
                regenerating: session
                    .as_ref()
                    .is_some_and(|session| session.regenerating()),
            };
            match &status {
                Some(ArtifactStatus::Described { source, summary }) => {
                    state.source = Some(source.into_bytes());
                    state.summary = summary.clone();
                }
                Some(ArtifactStatus::Failed(error)) => {
                    state.error = Some(error.clone());
                }
                Some(ArtifactStatus::Starting) => {}
                None => state.error = Some(unsupported.unwrap_or_default()),
            }
            let described = matches!(status, Some(ArtifactStatus::Described { .. }));
            if let Some(session) = session {
                if session.regenerating() {
                    host::request_repaint();
                }
                self.dynamic_artifact_sessions.insert(id, session);
            }
            if !described && self.dynamic_artifact_settings_open == Some(id) {
                self.dynamic_artifact_settings_open = None;
            }
            states.push(state);
        }
        if let Some(shell) = self.shell.and_then(|shell| self.editors.get(&shell)) {
            shell.set_artifact_states(states);
        }
        self.show_artifact_settings();
    }

    fn show_artifact_settings(&mut self) {
        let Some(id) = self.dynamic_artifact_settings_open else {
            surfaces::set_height(SurfaceId::ArtifactSettings, None);
            return;
        };
        let descriptor = self.client.dynamic_artifact(id);
        let (Some(descriptor), Some(mut session)) =
            (descriptor, self.dynamic_artifact_sessions.remove(&id))
        else {
            self.dynamic_artifact_settings_open = None;
            return;
        };
        let draft = self
            .dynamic_artifact_settings
            .entry(id)
            .or_insert_with(|| descriptor.data.clone());
        surfaces::set_height(SurfaceId::ArtifactSettings, Some(session.settings_height()));
        let registry = &self.registry;
        let client = &self.client;
        surfaces::with(SurfaceId::ArtifactSettings, |ui| {
            session.settings_ui(ui, registry, client, draft);
        });
        self.dynamic_artifact_sessions.insert(id, session);
    }

    fn apply_artifact_settings(&mut self) {
        let Some(id) = self.dynamic_artifact_settings_open else {
            return;
        };
        let Some(descriptor) = self.client.dynamic_artifact(id) else {
            self.dynamic_artifact_settings_open = None;
            return;
        };
        let Some(data) = self.dynamic_artifact_settings.remove(&id) else {
            return;
        };
        self.client.set_dynamic_artifact(
            id,
            DynamicArtifactDescriptor {
                source_type: descriptor.source_type,
                data: data.clone(),
            },
        );
        if let Some(session) = self.dynamic_artifact_sessions.get_mut(&id) {
            session.regenerate(&self.client, &data);
        }
        self.dynamic_artifact_errors.remove(&id);
        self.dynamic_artifact_settings_open = None;
    }

    fn cancel_artifact_settings(&mut self) {
        let Some(id) = self.dynamic_artifact_settings_open.take() else {
            return;
        };
        self.dynamic_artifact_settings.remove(&id);
        if let Some(session) = self.dynamic_artifact_sessions.get_mut(&id) {
            session.cancel_settings();
        }
    }

    fn unlink_artifact(&mut self) {
        let Some(id) = self.dynamic_artifact_unlink else {
            return;
        };
        self.client.clear_dynamic_artifact(id);
        self.dynamic_artifact_errors.remove(&id);
        self.forget_dynamic_artifact_dialogs(id);
    }

    fn handle_editor_action(&mut self, action: EditorAction) {
        match action {
            EditorAction::OpenBlock {
                id,
                block_type,
                via,
                from,
            } => self.show_in_shell(id, block_type, via, from),
            EditorAction::DragBlock { id, block_type } => {
                host::start_drag(host::DragPayload {
                    block_id: id,
                    block_type,
                });
            }
            EditorAction::Command { id, command } => self.handle_block_command(id, command),
        }
    }

    fn block_label(&self, id: Uuid) -> BlockLabel {
        self.client.cached_block(id).map_or_else(
            || {
                let block_type = self.block_type_of(id).unwrap_or_default();
                BlockLabel::new(&self.registry, block_type, None)
            },
            |cached| BlockLabel::for_cached(&self.registry, &cached),
        )
    }

    fn handle_block_command(&mut self, id: Uuid, command: BlockCommand) {
        match command {
            BlockCommand::Share => {
                let label = self.block_label(id);
                self.share.open(&self.client, id, label);
            }
            BlockCommand::Rename => {
                let name = self.block_label(id).name;
                self.rename = Some(RenameState { id, name });
            }
            BlockCommand::Undo if self.editor_access(id).can_edit() => be::undo(id),
            BlockCommand::Redo if self.editor_access(id).can_edit() => be::redo(id),
            BlockCommand::Undo | BlockCommand::Redo => {}
            BlockCommand::Unlink { container } => {
                self.queue_copy(id, Uuid::from_bytes(container));
            }
            BlockCommand::Artifact { action } => self.act_on_artifact(id, action),
            BlockCommand::CloseEditor => self.close_editor(id),
            BlockCommand::SimulateAccess { access } => {
                let access = match access {
                    AccessLevel::None => BlockAccess::None,
                    AccessLevel::KnowExists => BlockAccess::KnowExists,
                    AccessLevel::View => BlockAccess::View,
                    AccessLevel::Edit => BlockAccess::Edit,
                };
                match access == BlockAccess::Edit {
                    true => self.editor_access.remove(&id),
                    false => self.editor_access.insert(id, access),
                };
            }
            BlockCommand::Delete {
                block_type,
                source,
                is_reference,
            } => self.queue_delete(
                id,
                Uuid::from_bytes(block_type),
                drag_source(source),
                is_reference,
            ),
            BlockCommand::Move {
                block_type,
                source,
                destination,
                is_reference,
            } => self.queue_move(
                id,
                Uuid::from_bytes(block_type),
                drag_source(source),
                Uuid::from_bytes(destination),
                is_reference,
            ),
            BlockCommand::Place {
                block_type,
                parent,
                linked,
            } => self.queue_placement(
                id,
                Uuid::from_bytes(block_type),
                Uuid::from_bytes(parent),
                linked,
            ),
        }
    }

    fn frame(&mut self, context: &beui::Context) {
        if self.error.is_none() {
            self.error = panic_guard::take();
        }
        if self.error.is_some() {
            return;
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_frame(context);
        }));
        if caught.is_err() {
            self.error =
                Some(panic_guard::take().unwrap_or_else(|| "The app stopped responding.".into()));
        }
    }

    fn run_frame(&mut self, context: &beui::Context) {
        performance::begin_frame();
        plugin_host::poll();
        if !self.signed_in {
            be::stop();
            self.poll_account_request();
            performance::end_frame();
            return;
        }
        if self.scheduled_workspace_list {
            self.scheduled_workspace_list = false;
            let mut account = self.account.clone();
            account.last_workspace_id = None;
            let _ = self.app_state.set_last_workspace(&account, None);
            self.switch_account(account);
        }
        if let Some(account) = self.scheduled_account_switch.take() {
            self.switch_account(account);
        }
        if self.workspace.is_none() {
            be::stop();
            self.load_workspaces_if_needed();
            self.poll_reauth_request();
            performance::end_frame();
            return;
        }
        self.sync_ui_settings(context);
        self.sync_be_stack();
        self.poll_workspace_request();
        self.poll_reauth_request();
        self.process_pending_transfers();
        self.process_pending_copies();
        self.share.poll(&self.client);
        debug::poll(&self.client);
        self.show_shell();
        self.poll_artifacts();
        plugin_host::flush();
        performance::end_frame();
    }

    fn sync_be_stack(&mut self) {
        let Some(workspace) = self.workspace.as_ref().map(|workspace| workspace.id) else {
            return;
        };
        if be::installed_for(self.account.id, workspace) {
            return;
        }
        be::start(be::Config {
            server_url: self.server_url.clone(),
            token: self.account.token.clone(),
            account: self.account.id,
            workspace,
            #[cfg(not(target_arch = "wasm32"))]
            data_dir: self.data_dir.join("be-objects"),
        });
    }

    fn sync_ui_settings(&mut self, context: &beui::Context) {
        if self.ui_settings.is_none() {
            let Some(root_settings) = self.root_settings.find(&self.client) else {
                context.set_zoom_factor(1.0);
                return;
            };
            let Some(settings) = root_settings.read() else {
                return;
            };
            let Some(id) = settings
                .resolve(UiSettings::TYPE_ID, self.client_id)
                .and_then(|reference| reference.as_direct())
            else {
                context.set_zoom_factor(1.0);
                return;
            };
            self.ui_settings = Some(id);
        }
        let Some(id) = self.ui_settings else {
            return;
        };
        be::hold(id, UiSettings::TYPE_ID);
        let settings = be::content(id).and_then(|content| {
            <be_block::UiSettingsContent as be_block::BlockContent>::decode(&content.bytes).ok()
        });
        if let Some(settings) = settings {
            context.set_zoom_factor(settings.zoom());
        }
    }

    fn command(&mut self, context: &beui::Context, command: UiCommand) {
        match command {
            UiCommand::Restart => self.restart(),
            UiCommand::AskErrorAction(action) => self.pending_error_action = Some(action),
            UiCommand::CancelErrorAction => self.pending_error_action = None,
            UiCommand::ConfirmErrorAction => match self.pending_error_action.take() {
                Some(ErrorAction::DeleteClientDatabase) => self.delete_client_database(),
                #[cfg(not(target_arch = "wasm32"))]
                Some(ErrorAction::DeleteServerDatabase) => self.delete_server_database(),
                None => {}
            },
            UiCommand::Exit => std::process::exit(1),
            UiCommand::OpenAccount(key) => {
                if let Some(account) = self.account_by_key(&key) {
                    self.open_account(account, false);
                }
            }
            UiCommand::ChooseWorkspace(key) => {
                if let Some(account) = self.account_by_key(&key) {
                    self.open_account(account, true);
                }
            }
            UiCommand::LogOut(key) => {
                if let Some(account) = self.account_by_key(&key) {
                    self.log_out_account(&account);
                }
            }
            UiCommand::OpenAddAccount => {
                self.account_error = None;
                self.add_account_open = true;
                self.add_account_generation += 1;
            }
            UiCommand::SubmitAccount(form) => {
                if self.pending_account_request.is_none() {
                    self.begin_account_request(form);
                }
            }
            UiCommand::CloseAddAccount => {
                self.add_account_open = false;
                self.pending_account_request = None;
                self.account_error = None;
            }
            UiCommand::ReloadWorkspaces => {
                self.workspaces_loaded = false;
                self.workspaces_load_failed = false;
                self.begin_workspace_request(WorkspaceOperation::Load);
            }
            UiCommand::OpenWorkspace(id) => {
                if let Some(workspace) = self
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .cloned()
                {
                    self.open_workspace(workspace);
                }
            }
            UiCommand::RespondInvitation(id, accept) => {
                self.begin_workspace_request(WorkspaceOperation::Respond(id, accept));
            }
            UiCommand::CreateWorkspace(name) => {
                if !name.trim().is_empty() {
                    self.begin_workspace_request(WorkspaceOperation::Create(name));
                }
            }
            UiCommand::SwitchAccount | UiCommand::ManageAccounts => {
                self.signed_in = false;
                if let Err(error) = self.app_state.clear_active_account() {
                    self.account_error = Some(error.to_string());
                }
            }
            UiCommand::LogOutCurrent => {
                let account = self.account.clone();
                self.log_out_account(&account);
            }
            UiCommand::ReauthSubmit(password) => self.begin_reauth_request(password),
            UiCommand::ReauthLogOut => {
                if let Some(account) = self.reauth.take().map(|reauth| reauth.account) {
                    self.log_out_account(&account);
                }
                self.close_reauth();
            }
            UiCommand::ReauthClose => self.close_reauth(),
            UiCommand::OpenSettings => self.open_settings(),
            UiCommand::OpenInspector => self.inspector_requested = Some(true),
            UiCommand::InviteMember => self.invite_open = true,
            UiCommand::SwitchWorkspace => {
                if self.client.network_debug_snapshot().changes_saved {
                    self.scheduled_workspace_list = true;
                } else {
                    self.pending_destructive_action =
                        Some(PendingDestructiveAction::ChooseWorkspace);
                }
            }
            UiCommand::SwitchTo(key) => {
                if let Some(account) = self.account_by_key(&key) {
                    self.request_account_switch(account);
                }
            }
            UiCommand::About(open) => self.about_open = open,
            UiCommand::SendInvite(email, role) => {
                if let Some(workspace) = &self.workspace
                    && !email.trim().is_empty()
                {
                    let id = workspace.id;
                    self.begin_workspace_request(WorkspaceOperation::Invite(id, email, role));
                }
            }
            UiCommand::CloseInvite => self.invite_open = false,
            UiCommand::Discard => self.discard(context),
            UiCommand::CancelDiscard => self.pending_destructive_action = None,
            UiCommand::SubmitRename(name) => {
                if name.len() <= MAX_NAME_BYTES
                    && let Some(rename) = self.rename.take()
                {
                    self.client.set_block_name(rename.id, name);
                }
            }
            UiCommand::CancelRename => self.rename = None,
            UiCommand::ApplyArtifactSettings => self.apply_artifact_settings(),
            UiCommand::CancelArtifactSettings => self.cancel_artifact_settings(),
            UiCommand::Unlink => self.unlink_artifact(),
            UiCommand::CancelUnlink => self.dynamic_artifact_unlink = None,
            UiCommand::Share(command) => self.share.command(&self.client, command),
            UiCommand::Picker(command) => block_picker::deliver(command),
            UiCommand::Debug(command) => debug::command(&self.client, command),
        }
    }

    fn view(&self) -> AppView {
        let screen = match (&self.error, self.signed_in, &self.workspace) {
            (Some(_), _, _) => ui::Screen::Error,
            (None, false, _) => ui::Screen::Accounts,
            (None, true, None) => ui::Screen::Workspaces,
            (None, true, Some(_)) => ui::Screen::Workspace,
        };
        let changes_saved = match screen {
            ui::Screen::Workspace => self.client.network_debug_snapshot().changes_saved,
            _ => true,
        };
        let accounts: Vec<_> = self
            .accounts
            .iter()
            .map(|account| {
                ui::AccountRow::of(
                    account,
                    account.server == self.account.server && account.id == self.account.id,
                )
            })
            .collect();
        let workspace_name = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.name.clone())
            .unwrap_or_default();
        AppView {
            screen,
            error: ui::ErrorView {
                message: self.error.clone().unwrap_or_default(),
                pending: self.pending_error_action,
            },
            accounts: accounts.clone(),
            account_error: self.account_error.clone(),
            add_account: ui::AddAccountView {
                open: self.add_account_open,
                generation: self.add_account_generation,
                pending: self.pending_account_request.is_some(),
                error: self
                    .add_account_open
                    .then(|| self.account_error.clone())
                    .flatten(),
            },
            account: ui::AccountRow::of(&self.account, true),
            workspaces: ui::WorkspacesView {
                state: match (self.workspaces_loaded, self.workspaces_load_failed) {
                    (true, _) => ui::WorkspacesState::Loaded,
                    (false, true) => ui::WorkspacesState::Failed,
                    (false, false) => ui::WorkspacesState::Loading,
                },
                workspaces: self
                    .workspaces
                    .iter()
                    .map(|workspace| (workspace.id, workspace.name.clone()))
                    .collect(),
                invitations: self
                    .invitations
                    .iter()
                    .map(|invitation| ui::InvitationRow {
                        id: invitation.id,
                        workspace: invitation.workspace_name.clone(),
                        role: invitation.role.label().to_lowercase(),
                    })
                    .collect(),
                busy: self.pending_workspace_request.is_some(),
                error: self.workspace_error.clone(),
                created: self.workspace_created,
            },
            reauth: self.reauth.as_ref().map(|reauth| ui::ReauthView {
                email: reauth.account.email.clone(),
                busy: reauth.pending.is_some(),
                error: reauth.error.clone(),
            }),
            status: ui::StatusView {
                changes_saved,
                frame: performance::last_frame()
                    .map(|frame| {
                        format!(
                            "Frame {}: {:.3} ms",
                            frame.number,
                            frame.duration.as_secs_f64() * 1_000.0
                        )
                    })
                    .unwrap_or_default(),
                workspace: workspace_name.clone(),
                signed_in_as: format!("Signed in as {}", self.account.name),
                accounts,
            },
            invite: self.invite_open.then(|| ui::InviteView {
                workspace: workspace_name,
                busy: self.pending_workspace_request.is_some(),
                error: self.workspace_error.clone(),
                sent: self.invite_sent,
            }),
            about: self.about_open,
            discard: self.pending_destructive_action.as_ref().map(discard_view),
            rename: self.rename.as_ref().map(|rename| ui::RenameView {
                id: rename.id,
                name: rename.name.clone(),
            }),
            artifact_settings: self.dynamic_artifact_settings_open.and_then(|id| {
                let descriptor = self.client.dynamic_artifact(id)?;
                let draft = self.dynamic_artifact_settings.get(&id);
                let session = self.dynamic_artifact_sessions.get(&id);
                Some(ui::ArtifactSettingsView {
                    id,
                    changed: draft.is_some_and(|draft| *draft != descriptor.data),
                    summary: session
                        .zip(draft)
                        .and_then(|(session, draft)| session.summary(draft)),
                })
            }),
            unlink: self.dynamic_artifact_unlink.is_some(),
            share: self.share.view(&self.client),
            picker: block_picker::view(),
            presenting: surfaces::handle(SurfaceId::Presenting)
                .shown()
                .get_untracked(),
            debug: debug::view(&self.client),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn restart(&mut self) {
        block_client::shut_down_clients();
        match Self::new(Some(self.data_dir.clone())) {
            Ok(fresh) => *self = fresh,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn restart(&mut self) {
        block_client::shut_down_clients();
        match Self::new() {
            Ok(fresh) => *self = fresh,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn delete_client_database(&mut self) {
        let path = self.data_dir.join("app.sqlite3");
        let _ = std::mem::replace(&mut self.app_state, AppStateStore::placeholder());
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            self.error = Some(format!("failed to delete {}: {error}", path.display()));
            return;
        }
        self.restart();
    }

    #[cfg(target_arch = "wasm32")]
    fn delete_client_database(&mut self) {
        match AppStateStore::open().and_then(|store| store.clear()) {
            Ok(()) => self.restart(),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn delete_server_database(&mut self) {
        block_client::shut_down_clients();
        self.embedded_server = None;
        let path = self.data_dir.join("server");
        if let Err(error) = std::fs::remove_dir_all(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            self.error = Some(format!("failed to delete {}: {error}", path.display()));
            return;
        }
        self.restart();
    }
}

fn discard_view(action: &PendingDestructiveAction) -> ui::DiscardView {
    let (message, button) = match action {
        PendingDestructiveAction::Switch(account) => (
            format!(
                "Switching to {} will discard changes that have not reached the server.",
                account.name
            ),
            "Discard and switch",
        ),
        PendingDestructiveAction::ChooseWorkspace => (
            "Switching workspaces will discard changes that have not reached the server."
                .to_owned(),
            "Discard and switch",
        ),
        PendingDestructiveAction::Close => (
            "Closing Block Editor will discard changes that have not reached the server."
                .to_owned(),
            "Discard and close",
        ),
    };
    ui::DiscardView {
        title: "Discard unsaved changes?".to_owned(),
        message,
        button: button.to_owned(),
    }
}

fn drag_source(location: BlockLocation) -> SidebarDragSource {
    match location {
        BlockLocation::Root => SidebarDragSource::Root,
        BlockLocation::Orphaned => SidebarDragSource::Orphaned,
        BlockLocation::Block(id) => SidebarDragSource::Block(Uuid::from_bytes(id)),
    }
}
