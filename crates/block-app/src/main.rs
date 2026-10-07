mod accounts;
mod app_state;
mod be;
mod block_label;
mod compositor;
mod debug;
mod editors;
mod host;
mod input;
mod keys;
mod panic_guard;
mod performance;
mod platform;
mod plugin_host;
mod root_settings;
mod shell_route;
mod surfaces;
mod ui;
mod wayland;

use beui::styled::DocumentTheme;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    rc::Rc,
};

#[cfg(not(target_arch = "wasm32"))]
use std::{io, path::PathBuf};

use accounts::{AccountError, Session};
use app_state::{AppStateStore, SavedAccount, ServerLocation};
use be_block::{BlockContent, InputSettingsContent, UiSettingsContent, WORKSPACE_EDITOR};
use be_graph::{Access, BlockParent};
use be_protocol::{Workspace, WorkspaceInvitation, WorkspaceRole};
use beui::Document;
use block_plugin_api::{
    AccessLevel, ArtifactAction, BlockCommand, BlockLocation, HostPanel, ShellDialog, ShellRequest,
};
use editors::{
    ArtifactSession, ArtifactStatus, EditorAction, EditorRegistry, PluginEditor, SidebarDragSource,
    plugin::PickSource,
};
use root_settings::RootSettings;
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
    let mut app = BlockApp::new(None).map_err(|error| error.to_string())?;
    let mut options = run_options();
    let mut session = false;
    for argument in std::env::args().skip(1) {
        if argument == "--dev-workspace" {
            app.open_dev_workspace(None);
        } else if argument == "--session" && cfg!(target_os = "linux") {
            session = true;
            app.run_as_desktop();
        } else if let Some(path) = argument.strip_prefix("--accessibility-tree=") {
            options.accessibility_dump = Some(PathBuf::from(path));
        } else {
            return Err(format!("unknown argument {argument}").into());
        }
    }
    run_shell(options, Shell::new(app), session)
}

#[cfg(target_os = "linux")]
fn run_shell(options: beui::RunOptions, shell: Shell, session: bool) -> Result<(), Box<dyn Error>> {
    if session {
        return beui::run_on(Box::new(beui_adapter_drm::Drm), options, shell);
    }
    let renderer = beui::WindowRenderer::Wgpu {
        open_device: Some(std::sync::Arc::new(be_dmabuf::open_device)),
    };
    beui::run_with_renderers(options, vec![renderer], shell)
}

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
fn run_shell(
    options: beui::RunOptions,
    shell: Shell,
    _session: bool,
) -> Result<(), Box<dyn Error>> {
    beui::run_with(options, shell)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_web(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    panic_guard::install();
    editors::plugin::discovery::load().await;
    let mut app =
        BlockApp::new().map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let mut options = run_options();
    let location = web_sys::window()
        .map(|window| window.location())
        .ok_or("no browser window is available")?;
    let page = web_sys::Url::new(&location.href()?)?;
    if page.search_params().has("dev-workspace") {
        app.open_dev_workspace(Some(location.origin()?));
    }
    options.accessibility_tree = page.search_params().has("accessibility-tree");
    beui::run_web(
        &canvas_id,
        vec![beui::WebRenderer::Wgpu],
        options,
        Shell::new(app),
    )
    .await
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn accessibility_tree() -> Option<String> {
    beui::accessibility_tree()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: beui::AndroidApp) {
    editors::plugin::discovery::load(&app);
    panic_guard::install();
    let storage_root = app.internal_data_path();
    let options = run_options();
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
        let mut document = beui::reactive::build(|| {
            compositor::install();
            wayland::create();
            surfaces::create_handles();
            let store = AppViewStore::new(AppView::default());
            view = Some(store.clone());
            let view = store;
            beui::reactive::view! {
                <ui::Root view />
            }
        });
        document.on_interacted(plugin_host::start_frames);
        document.on_laid_out(|| {
            plugin_host::settle();
            compositor::notify_plugins();
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
        wayland::start(setup);
        input::start(setup);
        self.app.input.boot(&self.app.app_state);
        #[cfg(all(
            feature = "web-view",
            not(target_os = "android"),
            not(target_arch = "wasm32")
        ))]
        if let Some(window) = setup.get::<std::sync::Arc<beui::winit::window::Window>>() {
            plugin_host::install_web_view(window.clone());
        }
    }

    fn update(&mut self, context: &beui::Context, rect: beui::Rect) {
        let started = std::time::Instant::now();
        host::begin(context, &self.document);
        wayland::before(context, rect, &mut self.document);
        self.app.frame(context);
        if let Some(open) = self.app.inspector_requested.take()
            && open
        {
            self.document.open_inspector();
        }
        let view = self.app.view();
        let store = self.view.clone();
        beui::reactive::with_reactive_scope(&mut self.document, move || {
            compositor::notify();
            store.set(view);
        });
        #[cfg(target_arch = "wasm32")]
        plugin_host::place_screens(
            &plugin_host::shown_blits(),
            context.pixels_per_point() / context.native_pixels_per_point(),
            self.document.theme().background,
        );
        self.document.show(context, rect);
        wayland::after(context, &mut self.document);
        let commands = ui::take_commands();
        if !commands.is_empty() {
            for command in commands {
                self.app.command(context, command);
            }
            context.request_repaint();
        }
        host::end(context);
        performance::record_update(started, &self.document.performance().latest.timings);
        plugin_host::record_pacing();
    }

    fn clear_color(&self) -> beui::Color32 {
        self.document.theme().background
    }

    fn close_requested(&mut self) -> bool {
        self.app.close_requested()
    }

    fn exiting(&mut self) {
        wayland::exiting();
        be::flush();
        be::stop();
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
    root_settings: RootSettings,
    choosing_profile: bool,
    shell: Option<Uuid>,
    windows_sent: Option<(Uuid, u64)>,
    forwarded_picks: HashMap<u64, (PickSource, u64)>,
    next_pick: u64,
    focus_reports: HashMap<Uuid, editors::FocusReport>,
    artifact_watches: HashMap<Uuid, Vec<Uuid>>,
    ui_settings: Option<Uuid>,
    input: input::InputSync,
    block_types: HashMap<Uuid, Uuid>,
    registry: Rc<EditorRegistry>,
    editors: compositor::Editors,

    watched_artifacts: Vec<Uuid>,
    dynamic_artifact_sessions: HashMap<Uuid, Box<dyn ArtifactSession>>,
    dynamic_artifact_errors: HashMap<Uuid, String>,

    dynamic_artifact_settings: HashMap<Uuid, Vec<u8>>,

    dynamic_artifact_settings_open: Option<Uuid>,

    pending_transfers: Vec<PendingTransfer>,
    pending_copies: Vec<PendingCopy>,
    about_open: bool,
    run_program_open: bool,
    app_menu_open: bool,
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
    dev_workspace: bool,
    keys: keys::KeyState,
    workspace_key: Option<[u8; 32]>,
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
    Loaded(Vec<Workspace>, Vec<WorkspaceInvitation>, accounts::Keys),
    Created(Workspace),
    Responded,
    Invited,
}

type WorkspaceRequestError = AccountError;

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

impl BlockApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(storage_root: Option<PathBuf>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = storage_root
            .or_else(storage_dir)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?;
        std::fs::create_dir_all(&data_dir)?;
        panic_guard::report_in(&data_dir);
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
        let registry = Rc::new(EditorRegistry::new());
        let editors = compositor::Editors::install(Rc::clone(&registry), client_id);
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
            root_settings: RootSettings::new(WORKSPACE_EDITOR),
            choosing_profile: false,
            shell: None,
            windows_sent: None,
            forwarded_picks: HashMap::new(),
            focus_reports: HashMap::new(),
            artifact_watches: HashMap::new(),
            next_pick: 0,
            ui_settings: None,
            input: input::InputSync::default(),
            block_types: HashMap::new(),
            registry,
            editors,
            watched_artifacts: Vec::new(),
            dynamic_artifact_sessions: HashMap::new(),
            dynamic_artifact_errors: HashMap::new(),
            dynamic_artifact_settings: HashMap::new(),
            dynamic_artifact_settings_open: None,
            pending_transfers: Vec::new(),
            pending_copies: Vec::new(),
            about_open: false,
            run_program_open: false,
            app_menu_open: false,
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
            dev_workspace: false,
            keys: keys::KeyState::default(),
            workspace_key: None,
        })
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    fn run_as_desktop(&mut self) {
        self.root_settings = RootSettings::new(be_block::LINUX_DESKTOP_EDITOR);
    }

    #[cfg(not(target_os = "android"))]
    fn open_dev_workspace(&mut self, remote_url: Option<String>) {
        self.dev_workspace = true;
        if self.signed_in {
            return;
        }
        let server = match &remote_url {
            Some(url) => ServerLocation::Remote(url.clone()),
            None => ServerLocation::Local,
        };
        if let Some(account) = self
            .accounts
            .iter()
            .find(|account| account.server == server)
            .cloned()
        {
            self.switch_account(account);
            return;
        }
        self.begin_account_request(AccountForm {
            register: true,
            remote: remote_url.is_some(),
            remote_url: remote_url.unwrap_or_default(),
            email: "dev@localhost".to_owned(),
            display_name: "Dev".to_owned(),
            password: "dev-password".to_owned(),
        });
    }

    fn open_any_workspace(&mut self) {
        if !std::mem::take(&mut self.dev_workspace) || self.workspace.is_some() {
            return;
        }
        match self.workspaces.first().cloned() {
            Some(workspace) => self.open_workspace(workspace),
            None => {
                self.begin_workspace_request(WorkspaceOperation::Create("Dev".to_owned()));
                host::request_repaint();
            }
        }
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
        let url = requested_url.trim().trim_end_matches('/').to_owned();
        if url.is_empty() {
            self.account_error = Some("a server address is required".to_owned());
            return;
        }
        let server = if remote {
            ServerLocation::Remote(url.clone())
        } else {
            ServerLocation::Local
        };
        let requested = url.clone();
        let AccountForm {
            register,
            email,
            display_name,
            password,
            ..
        } = form;
        let dev = self.dev_workspace;
        let receiver = platform::spawn_request(async move {
            if !register {
                return accounts::login(requested, email, password)
                    .await
                    .map_err(|error| error.to_string());
            }
            match accounts::register(
                requested.clone(),
                email.clone(),
                display_name,
                password.clone(),
            )
            .await
            {
                Err(error) if dev => accounts::login(requested, email, password)
                    .await
                    .map_err(|_| error.to_string()),
                registered => registered.map_err(|error| error.to_string()),
            }
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
            return;
        };
        let pending = self.pending_account_request.take().unwrap();
        match result {
            Ok(session) => {
                let saved = SavedAccount {
                    server: pending.server,
                    id: session.account,
                    email: session.email,
                    name: session.display_name,
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
            let _ = accounts::logout(url, token).await;
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
        let url = self.server_url.clone();
        let token = self.account.token.clone();
        let receiver = platform::spawn_request(async move {
            match operation {
                WorkspaceOperation::Load => {
                    let (workspaces, invitations) =
                        accounts::workspaces(url.clone(), token.clone()).await?;
                    let keys = accounts::keys(url, token).await?;
                    Ok(WorkspaceResult::Loaded(workspaces, invitations, keys))
                }
                WorkspaceOperation::Create(name) => accounts::create_workspace(url, token, name)
                    .await
                    .map(WorkspaceResult::Created),
                WorkspaceOperation::Respond(invitation, accept) => {
                    accounts::respond_invitation(url, token, invitation, accept)
                        .await
                        .map(|()| WorkspaceResult::Responded)
                }
                WorkspaceOperation::Invite(workspace, email, role) => {
                    accounts::invite(url, token, workspace, email, role)
                        .await
                        .map(|()| WorkspaceResult::Invited)
                }
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
            return;
        };
        self.pending_workspace_request = None;
        match result {
            Ok(WorkspaceResult::Loaded(workspaces, invitations, keys)) => {
                self.workspaces = workspaces;
                self.invitations = invitations;
                self.workspaces_loaded = true;
                self.workspaces_load_failed = false;
                self.keys.loaded(keys);
                if self.keys.needs_recovery() {
                    if self.dev_workspace {
                        let held = self.held_keys();
                        self.keys.skip_confirmation(
                            self.server_url.clone(),
                            self.account.token.clone(),
                            held,
                        );
                    }
                    return;
                }
                if let Some(last_workspace_id) = self.account.last_workspace_id
                    && let Some(workspace) = self
                        .workspaces
                        .iter()
                        .find(|workspace| workspace.id == last_workspace_id)
                        .cloned()
                {
                    self.open_workspace(workspace);
                }
                self.open_any_workspace();
            }
            Ok(WorkspaceResult::Created(workspace)) => {
                self.workspaces.push(workspace.clone());
                self.workspace_created += 1;
                let key = *be_store::ContentKey::random().as_bytes();
                if let Err(error) =
                    self.app_state
                        .set_workspace_key(&self.account, workspace.id, key)
                {
                    self.workspace_error = Some(error.to_string());
                    return;
                }
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
        let receiver = platform::spawn_request(async move {
            accounts::login(url, email, password)
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
            return;
        };
        let Some(mut reauth) = self.reauth.take() else {
            return;
        };
        reauth.pending = None;
        match result {
            Ok(session) => {
                let mut updated = reauth.account.clone();
                updated.id = session.account;
                updated.email = session.email;
                updated.name = session.display_name;
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
        self.cancel_forwarded_picks();
        self.focus_reports.clear();
        self.artifact_watches.clear();
        be::stop();
        self.block_types.clear();
        self.registry = Rc::new(EditorRegistry::new());
        self.editors.reset(Rc::clone(&self.registry));
        self.watched_artifacts.clear();
        self.dynamic_artifact_sessions.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.root_settings = RootSettings::new(self.root_settings.shell());
        self.choosing_profile = false;
        self.shell = None;
        self.ui_settings = None;
        self.input.set_block(None);
        self.keys.cancel_pairing();
        self.workspace_key = match self.app_state.workspace_key(&self.account, workspace.id) {
            Ok(key) => key,
            Err(error) => {
                self.workspace_error = Some(error.to_string());
                None
            }
        };
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

    fn held_keys(&self) -> Vec<(Uuid, [u8; 32])> {
        self.app_state
            .workspace_keys(&self.account)
            .unwrap_or_default()
    }

    fn poll_keys(&mut self) {
        match self.keys.poll() {
            Some(keys::KeyEvent::RecoverySaved) if self.workspace.is_some() => {}
            Some(keys::KeyEvent::RecoverySaved) => {
                self.workspaces_loaded = false;
                self.workspaces_load_failed = false;
                self.begin_workspace_request(WorkspaceOperation::Load);
            }
            Some(keys::KeyEvent::Unlocked(workspace, key)) => self.unlocked(workspace, key),
            None => {}
        }
    }

    fn unlocked(&mut self, workspace: Uuid, key: [u8; 32]) {
        if let Err(error) = self
            .app_state
            .set_workspace_key(&self.account, workspace, key)
        {
            self.workspace_error = Some(error.to_string());
            return;
        }
        if self.workspace.as_ref().map(|open| open.id) == Some(workspace) {
            self.workspace_key = Some(key);
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
        if be::status().unsealed == 0 {
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
        self.cancel_forwarded_picks();
        self.focus_reports.clear();
        self.artifact_watches.clear();
        be::stop();
        self.block_types.clear();
        self.registry = Rc::new(EditorRegistry::new());
        self.editors.reset(Rc::clone(&self.registry));
        self.watched_artifacts.clear();
        self.dynamic_artifact_sessions.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.pending_transfers.clear();
        self.about_open = false;
        self.run_program_open = false;
        self.app_menu_open = false;
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
        self.root_settings = RootSettings::new(self.root_settings.shell());
        self.choosing_profile = false;
        self.shell = None;
        self.ui_settings = None;
        self.input.set_block(None);
        self.keys = keys::KeyState::default();
        self.workspace_key = None;
        self.account = account;
        self.server_url = server_url;
        self.signed_in = true;
        if let Err(error) = self.app_state.set_active_account(&self.account) {
            self.account_error = Some(error.to_string());
        }
    }

    fn close_requested(&mut self) -> bool {
        be::flush();
        if self.allow_close || be::status().unsealed == 0 {
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
            be::node(id)
                .map(|node| node.content_type)
                .or_else(|| self.with_editor(id, |editor| editor.block_type()))
        })
    }

    fn ensure_editor(&mut self, id: Uuid) -> bool {
        if self.editors.with(|open| open.contains_key(&id)) {
            return true;
        }
        let Some(block_type) = self.block_type_of(id) else {
            return false;
        };
        self.block_types.insert(id, block_type);
        let editor = self.registry.open(id, block_type);
        self.editors.with(|open| open.insert(id, editor));
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
            parent_after: (!linked).then_some(BlockParent::Block(parent)),
            stage: TransferStage::AddDestination,
        });
    }

    fn set_block_parent(&mut self, id: Uuid, parent: BlockParent) {
        be::set_parent(id, parent);
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
            parent_after: (!is_reference).then_some(BlockParent::Block(destination)),
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
                .then_some(BlockParent::Detached),
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
                        be::set_parent(transfer.child, BlockParent::Detached);
                        Some(true)
                    }
                    Some(SidebarDragSource::Block(source)) => self
                        .with_editor(source, |editor| editor.delete_child(transfer.child))
                        .flatten(),
                };
                if ready != Some(true) {
                    self.pending_transfers.push(transfer);
                    continue;
                }
                transfer.stage = TransferStage::AddDestination;
            }

            let ready = transfer.destination.map_or(Some(true), |destination| {
                self.with_editor(destination, |editor| editor.add_child(transfer.child))
                    .flatten()
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
                let Some((copy_id, block_type)) = be::duplicate(copy.source) else {
                    self.pending_copies.push(copy);
                    continue;
                };
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
                .with_editor(copy.container, |editor| {
                    editor.replace_child(copy.source, copy_id)
                })
                .flatten();
            if replaced != Some(true) {
                self.pending_copies.push(copy);
                continue;
            }

            self.set_block_parent(copy_id, BlockParent::Block(copy.container));
            self.show_block(
                Some(copy.container),
                copy_id,
                block_type,
                Some(copy.container),
            );
        }
    }

    fn editor_access_ceiling(&self, id: Uuid) -> Access {
        editors::editor_access_ceiling(id)
    }

    fn editor_access(&self, id: Uuid) -> Access {
        let ceiling = self.editor_access_ceiling(id);
        self.editors
            .simulated(id)
            .map_or(ceiling, |chosen| chosen.min(ceiling))
    }

    fn forget_dynamic_artifact_dialogs(&mut self, id: Uuid) {
        self.dynamic_artifact_settings.remove(&id);
        if self.dynamic_artifact_settings_open == Some(id) {
            self.dynamic_artifact_settings_open = None;
        }
    }

    fn ensure_shell(&mut self) -> Option<Uuid> {
        let id = self.root_settings.ensure_profile(self.client_id)?;
        if let Some(previous) = self.shell.filter(|previous| *previous != id) {
            self.shell = None;
            let open: Vec<Uuid> = self.editors.with(|open| open.keys().copied().collect());
            for editor in open {
                self.close_editor(editor);
            }
            self.block_types.remove(&previous);
        }
        let shell_editor = self.root_settings.shell();
        self.block_types.insert(id, shell_editor);
        if !self.editors.with(|open| open.contains_key(&id)) {
            let editor = self.registry.open(id, shell_editor).viewed_by(Some(id));
            if let Some(missing) = ShellRequest::ALL
                .into_iter()
                .find(|request| !editor.accepts(*request))
            {
                eprintln!("the shell editor does not accept {missing:?} requests");
            }
            self.editors.with(|open| open.insert(id, editor));
            self.windows_sent = None;
        }
        self.shell = Some(id);
        Some(id)
    }

    fn handler_for(&self, from: Option<Uuid>, request: ShellRequest) -> Option<Uuid> {
        Some(shell_route::handler(
            from,
            self.shell?,
            |id| self.with_editor(id, |editor| editor.accepts(request)) == Some(true),
            |id| self.editors.parent_of(id),
        ))
    }

    fn show_dialog(&mut self, from: Option<Uuid>, id: Uuid, dialog: ShellDialog) {
        if let Some(handler) = self.handler_for(from, ShellRequest::Dialog) {
            self.with_editor(handler, |handler| handler.show_dialog(id, dialog));
        }
    }

    fn show_panel(&mut self, panel: HostPanel) {
        if let Some(shell) = self.shell {
            self.with_editor(shell, |shell| shell.show_panel(panel));
        }
    }

    fn show_block(&mut self, from: Option<Uuid>, id: Uuid, block_type: Uuid, via: Option<Uuid>) {
        self.block_types.insert(id, block_type);
        if let Some(handler) = self.handler_for(from, ShellRequest::ShowBlock) {
            self.with_editor(handler, |handler| handler.show_block(id, block_type, via));
        }
    }

    fn close_editor(&mut self, id: Uuid) {
        if self.shell == Some(id) {
            return;
        }
        self.editors.forget_parent(id);
        self.focus_reports.remove(&id);
        if self.artifact_watches.remove(&id).is_some() {
            self.watch_artifacts(self.watched());
        }
        self.editors
            .with_simulated(|simulated| simulated.remove(&id));
        self.dynamic_artifact_sessions.remove(&id);
        self.dynamic_artifact_errors.remove(&id);
        self.forget_dynamic_artifact_dialogs(id);
        self.watched_artifacts.retain(|watched| *watched != id);
        if let Some(mut editor) = self.editors.with(|open| open.remove(&id)) {
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
                let data = artifact_of(id).map(|artifact| artifact.data);
                if let (Some(data), Some(session)) =
                    (data, self.dynamic_artifact_sessions.get_mut(&id))
                {
                    session.regenerate(&data);
                    self.dynamic_artifact_errors.remove(&id);
                }
            }
            ArtifactAction::Unlink => self.unlink_artifact(id),
        }
    }

    fn with_editor<R>(&self, id: Uuid, act: impl FnOnce(&mut PluginEditor) -> R) -> Option<R> {
        self.editors.with(|open| open.get_mut(&id).map(act))
    }

    fn show_shell(&mut self) {
        let Some(shell) = self.ensure_shell() else {
            return;
        };
        compositor::set_shell(Some(shell));
        let windows = (Some((shell, wayland::revision())) != self.windows_sent).then(|| {
            self.windows_sent = Some((shell, wayland::revision()));
            wayland::listed()
        });
        let Some(closed) = self.with_editor(shell, |editor| {
            if let Some(windows) = windows {
                editor.set_windows(windows);
            }
            editor.take_closed_windows()
        }) else {
            return;
        };
        for window in closed {
            wayland::close(window);
        }
        let reports: Vec<(Uuid, Option<editors::FocusReport>, Option<Vec<Uuid>>)> =
            self.editors.with(|open| {
                open.values()
                    .map(|editor| {
                        (
                            editor.id(),
                            editor.take_focus_report(),
                            editor.take_artifact_watch(),
                        )
                    })
                    .collect()
            });
        let mut refocused = false;
        let mut rewatched = false;
        for (id, focus, watch) in reports {
            if let Some(focus) = focus {
                self.focus_reports.insert(id, focus);
                refocused = true;
            }
            if let Some(watch) = watch {
                self.artifact_watches.insert(id, watch);
                rewatched = true;
            }
        }
        if refocused && let Some(focus) = self.focus(shell) {
            plugin_host::set_focus(focus.block, focus.via);
        }
        if rewatched {
            self.watch_artifacts(self.watched());
        }
        if host::key_pressed(beui::Key::Escape) {
            let presenting: Vec<Uuid> = self.editors.with(|open| {
                open.values()
                    .filter(|editor| editor.presenting_now())
                    .map(PluginEditor::id)
                    .collect()
            });
            for id in presenting {
                self.with_editor(id, PluginEditor::stop_presenting_now);
            }
        }
        self.forward_block_picks(shell);
        self.editors.step_creations();
        for (from, action) in compositor::take_actions() {
            self.handle_editor_action(from, action);
        }
    }

    fn focus(&self, shell: Uuid) -> Option<editors::FocusReport> {
        let mut report = self.focus_reports.get(&shell)?;
        let mut visited = HashSet::from([shell]);
        while let Some((block, _)) = report.block
            && visited.insert(block)
            && let Some(inner) = self.focus_reports.get(&block)
            && inner.block.is_some()
        {
            report = inner;
        }
        Some(editors::FocusReport {
            block: report.block,
            via: report.via.clone(),
        })
    }

    fn watched(&self) -> Vec<Uuid> {
        let mut watched: Vec<Uuid> = self.artifact_watches.values().flatten().copied().collect();
        watched.sort_unstable();
        watched.dedup();
        watched
    }

    fn cancel_forwarded_picks(&mut self) {
        for (_, (source, request_id)) in self.forwarded_picks.drain() {
            source.answer(request_id, block_plugin_api::BlockPick::Cancelled);
        }
    }

    fn forward_block_picks(&mut self, shell: Uuid) {
        let editors: Vec<(Uuid, PickSource, bool)> = self.editors.with(|open| {
            open.values()
                .filter_map(|editor| {
                    Some((
                        editor.id(),
                        editor.pick_source()?,
                        editor.accepts(ShellRequest::Pick),
                    ))
                })
                .collect()
        });
        for (_, picker, _) in editors.iter().filter(|(_, _, accepts)| *accepts) {
            for (pick, answer) in
                crate::plugin_host::take_pick_answers(&picker.plugin_id, picker.instance)
            {
                if let Some((source, request_id)) = self.forwarded_picks.remove(&pick) {
                    source.answer(request_id, answer);
                }
            }
        }
        let pickers: HashMap<Uuid, PickSource> = editors
            .iter()
            .filter(|(_, _, accepts)| *accepts)
            .map(|(id, picker, _)| (*id, picker.clone()))
            .collect();
        let editors: Vec<(Uuid, PickSource)> = editors
            .into_iter()
            .map(|(id, source, _)| (id, source))
            .collect();
        for (id, source) in &editors {
            let (settings, commits): (Vec<_>, Vec<_>) =
                crate::plugin_host::take_child_commits(&source.plugin_id, source.instance)
                    .into_iter()
                    .partition(|commit| self.editors.settings_block(*id, commit.child).is_some());
            for commit in settings {
                if let Some(block) = self.editors.settings_block(*id, commit.child) {
                    self.apply_artifact_settings(block);
                }
            }
            if !commits.is_empty() {
                self.editors.commit_creations(*id, commits);
            }
        }
        self.editors.forget_hidden_settings();
        let creations = self.editors.creation_pick_sources();
        let requesters = editors
            .into_iter()
            .map(|(id, source)| (Some(id), source))
            .chain(creations.into_iter().map(|source| (None, source)));
        for (block, source) in requesters {
            while let Some(request) = source.take() {
                let mut filter = request.filter;
                filter.excluded.extend(block.map(Uuid::into_bytes));
                let parent = match block {
                    Some(block) => block_plugin_api::BlockLocation::Block(block.into_bytes()),
                    None => block_plugin_api::BlockLocation::Root,
                };
                self.next_pick += 1;
                let pick = self.next_pick;
                let handler = self.handler_for(block, ShellRequest::Pick).unwrap_or(shell);
                let Some(picker) = pickers.get(&handler).or_else(|| pickers.get(&shell)) else {
                    source.answer(request.request_id, block_plugin_api::BlockPick::Cancelled);
                    continue;
                };
                self.forwarded_picks
                    .insert(pick, (source.clone(), request.request_id));
                crate::plugin_host::request_pick(
                    &picker.plugin_id,
                    picker.instance,
                    pick,
                    filter,
                    parent,
                );
            }
        }
    }

    fn poll_artifacts(&mut self) {
        let watched = self.watched_artifacts.clone();
        let mut states = Vec::new();
        for id in watched {
            let Some(descriptor) = artifact_of(id) else {
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
                .map(|session| session.poll(&self.registry, &descriptor.data));
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
        let watchers: Vec<Uuid> = self.artifact_watches.keys().copied().collect();
        for watcher in watchers {
            self.with_editor(watcher, |editor| editor.set_artifact_states(states.clone()));
        }
        self.show_artifact_settings();
    }

    fn show_artifact_settings(&mut self) {
        let requested = self.editors.requested_settings();
        if self.dynamic_artifact_settings_open != requested {
            self.cancel_artifact_settings();
            self.dynamic_artifact_settings_open = requested;
        }
        let Some(id) = self.dynamic_artifact_settings_open else {
            self.editors.set_settings_status(None);
            return;
        };
        let descriptor = artifact_of(id);
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
        surfaces::host(
            SurfaceId::ArtifactSettings,
            Some(session.settings_region(&self.registry)),
        );
        if let Some(edited) = session.take_draft() {
            *draft = edited;
        }
        let progress = block_plugin_api::SettingsProgress {
            changed: *draft != descriptor.data,
            summary: session.summary(draft),
        };
        self.editors
            .set_settings_status(Some((id, progress, session.settings_height())));
        self.dynamic_artifact_sessions.insert(id, session);
    }

    fn apply_artifact_settings(&mut self, id: Uuid) {
        if self.dynamic_artifact_settings_open != Some(id) {
            return;
        }
        let Some(descriptor) = artifact_of(id) else {
            self.dynamic_artifact_settings_open = None;
            return;
        };
        let Some(data) = self.dynamic_artifact_settings.remove(&id) else {
            return;
        };
        set_artifact(
            id,
            Some(be_block::ArtifactSource {
                source_type: descriptor.source_type,
                data: data.clone(),
            }),
        );
        if let Some(session) = self.dynamic_artifact_sessions.get_mut(&id) {
            session.regenerate(&data);
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

    fn unlink_artifact(&mut self, id: Uuid) {
        set_artifact(id, None);
        self.dynamic_artifact_errors.remove(&id);
        self.forget_dynamic_artifact_dialogs(id);
    }

    fn handle_editor_action(&mut self, from: Option<Uuid>, action: EditorAction) {
        match action {
            EditorAction::OpenBlock {
                id,
                block_type,
                via,
            } => self.show_block(from, id, block_type, via),
            EditorAction::DragBlock { id, block_type } => {
                host::start_drag(host::DragPayload {
                    block_id: id,
                    block_type,
                });
            }
            EditorAction::Command { id, command } => self.handle_block_command(from, id, command),
        }
    }

    fn handle_block_command(&mut self, from: Option<Uuid>, id: Uuid, command: BlockCommand) {
        match command {
            BlockCommand::Share => self.show_dialog(from, id, ShellDialog::Share),
            BlockCommand::Rename => self.show_dialog(from, id, ShellDialog::Rename),
            BlockCommand::Undo if self.editor_access(id).can_edit() => be::undo(id),
            BlockCommand::Redo if self.editor_access(id).can_edit() => be::redo(id),
            BlockCommand::Undo | BlockCommand::Redo => {}
            BlockCommand::AppMenu => self.app_menu_open = true,
            BlockCommand::Unlink { container } => {
                self.queue_copy(id, Uuid::from_bytes(container));
            }
            BlockCommand::Artifact { action } => self.act_on_artifact(id, action),
            BlockCommand::CloseEditor => self.close_editor(id),
            BlockCommand::SimulateAccess { access } => {
                let access = match access {
                    AccessLevel::None => Access::None,
                    AccessLevel::KnowExists => Access::KnowExists,
                    AccessLevel::View => Access::View,
                    AccessLevel::Edit => Access::Edit,
                };
                match access == Access::Edit {
                    true => self
                        .editors
                        .with_simulated(|simulated| simulated.remove(&id)),
                    false => self
                        .editors
                        .with_simulated(|simulated| simulated.insert(id, access)),
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
        if self.error.is_none()
            && let Some(report) = panic_guard::take()
        {
            self.crashed(report);
        }
        if self.error.is_some() {
            return;
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_frame(context);
        }));
        if caught.is_err() {
            self.crashed(
                panic_guard::take().unwrap_or_else(|| "The app stopped responding.".into()),
            );
        }
    }

    fn crashed(&mut self, report: String) {
        self.error = Some(report);
        let _ = self.app_state.clear_active_account();
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
        self.poll_keys();
        if self.workspace.is_none() {
            be::stop();
            self.load_workspaces_if_needed();
            self.poll_reauth_request();
            performance::end_frame();
            return;
        }
        if self.workspace_key.is_none() {
            be::stop();
            if !self.workspaces_loaded {
                self.load_workspaces_if_needed();
            }
            self.poll_workspace_request();
            self.poll_reauth_request();
            performance::end_frame();
            return;
        }
        self.sync_ui_settings(context);
        self.sync_input_settings();
        self.sync_be_stack();
        self.poll_workspace_request();
        self.poll_reauth_request();
        if self.choosing_profile {
            self.root_settings.ensure();
            performance::end_frame();
            return;
        }
        self.process_pending_transfers();
        self.process_pending_copies();
        debug::poll();
        self.show_shell();
        self.poll_artifacts();
        plugin_host::flush();
        performance::end_frame();
    }

    fn sync_be_stack(&mut self) {
        let Some(workspace) = self.workspace.as_ref().map(|workspace| workspace.id) else {
            return;
        };
        let Some(content_key) = self.workspace_key else {
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
            content_key,
            other_keys: self
                .held_keys()
                .into_iter()
                .filter(|(held, _)| *held != workspace)
                .collect(),
            #[cfg(not(target_arch = "wasm32"))]
            data_dir: self.data_dir.join("be-objects"),
        });
    }

    fn sync_input_settings(&mut self) {
        if self.input.block().is_none() {
            let block = self
                .root_settings
                .find()
                .and_then(root_settings::settings)
                .and_then(|settings| {
                    settings.resolve(InputSettingsContent::CONTENT_TYPE, self.client_id)
                });
            if block.is_none() {
                return;
            }
            self.input.set_block(block);
        }
        self.input.sync(&self.app_state);
    }

    fn sync_ui_settings(&mut self, context: &beui::Context) {
        if self.ui_settings.is_none() {
            let Some(root_settings) = self.root_settings.find() else {
                context.set_zoom_factor(1.0);
                return;
            };
            let Some(settings) = root_settings::settings(root_settings) else {
                return;
            };
            let Some(id) = settings.resolve(UiSettingsContent::CONTENT_TYPE, self.client_id) else {
                context.set_zoom_factor(1.0);
                return;
            };
            self.ui_settings = Some(id);
        }
        let Some(id) = self.ui_settings else {
            return;
        };
        be::hold(id, UiSettingsContent::CONTENT_TYPE);
        let settings = be::content(id).and_then(|content| {
            <be_block::UiSettingsContent as be_block::BlockContent>::decode(&content.bytes).ok()
        });
        if let Some(settings) = settings {
            context.set_zoom_factor(settings.root().zoom());
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
            UiCommand::ChooseProfile(id) => {
                if let Some(workspace) = self
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .cloned()
                {
                    self.open_workspace(workspace);
                    self.choosing_profile = true;
                }
            }
            UiCommand::OpenProfile(profile) => {
                self.root_settings.use_profile(self.client_id, profile);
                self.choosing_profile = false;
            }
            UiCommand::OpenNewProfile => {
                self.root_settings.new_profile(self.client_id);
                self.choosing_profile = false;
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
            UiCommand::SwitchProfile(profile) => {
                self.root_settings.use_profile(self.client_id, profile);
            }
            UiCommand::NewProfile => self.root_settings.new_profile(self.client_id),
            UiCommand::OpenInspector => self.inspector_requested = Some(true),
            UiCommand::InviteMember => self.invite_open = true,
            UiCommand::SwitchWorkspace => {
                if be::status().unsealed == 0 {
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
            UiCommand::RunProgram(open) => self.run_program_open = open,
            UiCommand::Launch(command) => {
                self.run_program_open = false;
                wayland::launch(command);
            }
            UiCommand::AppMenu(open) => self.app_menu_open = open,
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
            UiCommand::Debug(command) => debug::command(command),
            UiCommand::ShowPanel(panel) => self.show_panel(panel),
            UiCommand::ConfirmRecovery(words) => {
                let held = self.held_keys();
                self.keys.confirm_recovery(
                    &words,
                    self.server_url.clone(),
                    self.account.token.clone(),
                    held,
                );
            }
            UiCommand::UnlockWithPhrase(phrase) => {
                if let Some(workspace) = self.workspace.as_ref().map(|workspace| workspace.id)
                    && let Some(key) = self.keys.unlock_with_phrase(workspace, &phrase)
                {
                    self.unlocked(workspace, key);
                }
            }
            UiCommand::StartPairing => {
                if let Some(workspace) = self.workspace.as_ref().map(|workspace| workspace.id) {
                    self.keys.start_pairing(
                        self.server_url.clone(),
                        self.account.token.clone(),
                        workspace,
                    );
                }
            }
            UiCommand::CancelPairing => self.keys.cancel_pairing(),
            UiCommand::NewRecoveryPhrase => self.keys.replace_recovery(),
            UiCommand::CancelRecovery => self.keys.cancel_replacing(),
            UiCommand::ApprovePairing(from, code) => be::approve_pairing(from, code),
            UiCommand::DismissPairing(from) => be::dismiss_pairing(from),
        }
    }

    fn view(&self) -> AppView {
        let screen = match (&self.error, self.signed_in, &self.workspace) {
            (Some(_), _, _) => ui::Screen::Error,
            (None, false, _) => ui::Screen::Accounts,
            (None, true, _) if self.keys.replacing() => ui::Screen::Recovery,
            (None, true, None) if self.keys.needs_recovery() => ui::Screen::Recovery,
            (None, true, None) => ui::Screen::Workspaces,
            (None, true, Some(_)) if self.workspace_key.is_none() => ui::Screen::Unlock,
            (None, true, Some(_)) if self.choosing_profile => ui::Screen::Profiles,
            (None, true, Some(_)) => ui::Screen::Workspace,
        };
        let changes_saved = match screen {
            ui::Screen::Workspace => be::status().unsealed == 0,
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
                unsaved: be::status().unsealed,
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
            recovery: self.keys.recovery_view(),
            unlock: self
                .workspace
                .as_ref()
                .map(|workspace| self.keys.unlock_view(workspace.id, &workspace.name))
                .unwrap_or_default(),
            pairing: match screen {
                ui::Screen::Workspace => be::pairing_requests()
                    .into_iter()
                    .map(|request| ui::PairingRow {
                        from: request.from,
                        device: request.device,
                        workspace: self
                            .workspaces
                            .iter()
                            .find(|workspace| workspace.id == request.workspace)
                            .map(|workspace| workspace.name.clone())
                            .unwrap_or_else(|| "another workspace".to_owned()),
                    })
                    .collect(),
                _ => Vec::new(),
            },
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
                workspace: workspace_name.clone(),
                signed_in_as: format!("Signed in as {}", self.account.name),
                accounts,
                profiles: self
                    .root_settings
                    .profiles(self.client_id)
                    .into_iter()
                    .map(|(id, name, current)| ui::ProfileRow { id, name, current })
                    .collect(),
                profiles_loaded: self.root_settings.loaded(),
                runs_programs: wayland::running(),
            },
            invite: self.invite_open.then(|| ui::InviteView {
                workspace: workspace_name,
                busy: self.pending_workspace_request.is_some(),
                error: self.workspace_error.clone(),
                sent: self.invite_sent,
            }),
            about: self.about_open,
            run_program: self.run_program_open,
            app_menu: self.app_menu_open,
            discard: self.pending_destructive_action.as_ref().map(discard_view),
            presenting: self
                .editors
                .with(|open| open.values().any(PluginEditor::presenting_now)),
            debug: debug::view(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn restart(&mut self) {
        be::flush();
        be::stop();
        match Self::new(Some(self.data_dir.clone())) {
            Ok(fresh) => *self = fresh,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn restart(&mut self) {
        be::flush();
        be::stop();
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
        be::stop();
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

fn artifact_of(id: Uuid) -> Option<be_block::ArtifactSource> {
    be::node(id)?.metadata.artifact
}

fn set_artifact(id: Uuid, artifact: Option<be_block::ArtifactSource>) {
    let Some(mut metadata) = be::node(id).map(|node| node.metadata) else {
        return;
    };
    metadata.artifact = artifact;
    be::set_metadata(id, metadata);
}

fn drag_source(location: BlockLocation) -> SidebarDragSource {
    match location {
        BlockLocation::Root => SidebarDragSource::Root,
        BlockLocation::Detached => SidebarDragSource::Orphaned,
        BlockLocation::Block(id) => SidebarDragSource::Block(Uuid::from_bytes(id)),
    }
}
