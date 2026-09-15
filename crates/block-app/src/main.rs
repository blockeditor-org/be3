mod app_state;
mod block_picker;
mod debug;
mod editors;
mod files;
mod panic_guard;
mod performance;
mod platform;
mod plugin_host;
mod share;
mod slide_templates;

use std::{collections::HashMap, error::Error, sync::Arc, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
use std::{io, path::PathBuf};

use app_state::{AppStateStore, SavedAccount, ServerLocation};
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
    PluginEditor, SidebarDragPayload, SidebarDragSource, direct_editor_tab_ui,
};
use eframe::egui;
use egui_material_icons::icons::{
    ICON_ADD, ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_CLOUD, ICON_COMPUTER, ICON_GROUP_ADD,
    ICON_KEYBOARD_ARROW_DOWN, ICON_LOGOUT, ICON_MORE_HORIZ, ICON_REFRESH, ICON_SWITCH_ACCOUNT,
    ICON_WORKSPACES,
};
use share::ShareDialog;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
const APP_ID: &str = "Block";
const ONBOARDING_WIDTH: f32 = 460.0;

pub(crate) const COMMIT: &str = env!("BLOCK_APP_COMMIT");
#[cfg(not(target_arch = "wasm32"))]
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native(options: eframe::NativeOptions, storage_root: Option<PathBuf>) -> eframe::Result {
    panic_guard::install();
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation_context| {
            egui_material_icons::initialize(&creation_context.egui_ctx);
            plugin_host::install(creation_context);
            BlockApp::new(storage_root).map(|app| Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run() -> eframe::Result {
    run_native(native_options(), None)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_web(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    wasi_threads::initialize_main_thread();
    panic_guard::install();

    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("no browser document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    editors::plugin::discovery::load().await;

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|creation_context| {
                egui_material_icons::initialize(&creation_context.egui_ctx);
                plugin_host::install(creation_context);
                BlockApp::new()
                    .map(|app| Box::new(app) as Box<dyn eframe::App>)
                    .map_err(Into::into)
            }),
        )
        .await
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    editors::plugin::discovery::load(&app);
    let storage_root = app.internal_data_path();
    let mut options = native_options();
    options.android_app = Some(app);
    let exit_code = match run_native(options, storage_root) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Block stopped: {error}");
            1
        }
    };

    std::process::exit(exit_code);
}

struct BlockApp {
    app_state: AppStateStore,

    client_id: Uuid,
    local_server_url: String,
    accounts: Vec<Account>,
    signed_in: bool,
    account_form: AccountForm,
    add_account_open: bool,
    pending_account_request: Option<PendingAccountRequest>,
    account_error: Option<String>,
    workspace: Option<Workspace>,
    workspaces: Vec<Workspace>,
    invitations: Vec<WorkspaceInvitation>,
    workspaces_loaded: bool,
    workspaces_load_failed: bool,
    pending_workspace_request:
        Option<platform::RequestResult<Result<WorkspaceResult, WorkspaceRequestError>>>,
    workspace_name: String,
    workspace_error: Option<String>,

    reauth: Option<ReauthState>,
    invite_open: bool,
    invite_email: String,
    invite_role: WorkspaceRole,
    scheduled_workspace_list: bool,
    server_url: String,
    account: Account,
    client: Arc<BlockClient>,
    root_settings: RootSettings,
    file_tree: RootSetting<FileTree>,
    workspace_ui: RootSetting<WorkspaceUi>,
    shell: Option<Uuid>,
    ui_settings: Option<BlockHandle<UiSettings>>,
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
    client_debug_open: bool,
    network_debug_open: bool,
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
}

#[derive(Clone)]
enum ErrorAction {
    DeleteClientDatabase,
    #[cfg(not(target_arch = "wasm32"))]
    DeleteServerDatabase,
}

type Account = SavedAccount;

struct AccountForm {
    remote: bool,
    remote_url: String,
    register: bool,
    email: String,
    display_name: String,
    password: String,
}

impl Default for AccountForm {
    fn default() -> Self {
        Self {
            remote: !platform::HAS_EMBEDDED_SERVER,
            remote_url: String::new(),
            register: false,
            email: String::new(),
            display_name: String::new(),
            password: String::new(),
        }
    }
}

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
    password: String,
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
            .or_else(|| eframe::storage_dir(APP_ID))
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
            account_form: AccountForm::default(),
            add_account_open: false,
            pending_account_request: None,
            account_error: None,
            workspace: None,
            workspaces: Vec::new(),
            invitations: Vec::new(),
            workspaces_loaded: false,
            workspaces_load_failed: false,
            pending_workspace_request: None,
            workspace_name: String::new(),
            workspace_error: None,
            reauth: None,
            invite_open: false,
            invite_email: String::new(),
            invite_role: WorkspaceRole::Editor,
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
            client_debug_open: false,
            network_debug_open: false,
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
        })
    }

    fn begin_account_request(&mut self) {
        let requested_url = if self.account_form.remote {
            self.account_form.remote_url.clone()
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
        let server = if self.account_form.remote {
            ServerLocation::Remote(client.url().to_owned())
        } else {
            ServerLocation::Local
        };
        let url = client.url().to_owned();
        let register = self.account_form.register;
        let email = self.account_form.email.clone();
        let display_name = self.account_form.display_name.clone();
        let password = self.account_form.password.clone();
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

    fn poll_account_request(&mut self, ctx: &egui::Context) {
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
                self.account_form = AccountForm::default();
                self.add_account_open = false;
                self.switch_account(ctx, saved);
            }
            Err(error) => self.account_error = Some(error),
        }
    }

    fn show_account_onboarding(&mut self, ui: &mut egui::Ui) {
        self.poll_account_request(ui.ctx());
        let mut action = None;
        let mut add_account = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            onboarding_column(ui, |ui| {
                ui.add_space(36.0);
                ui.heading("Block Editor");
                ui.weak("Choose an account to continue.");
                ui.add_space(20.0);

                for account in &self.accounts {
                    if let Some(chosen) = show_account_card(ui, account) {
                        action = Some(chosen);
                    }
                    ui.add_space(8.0);
                }
                if self.accounts.is_empty() {
                    onboarding_card(ui, |ui| {
                        ui.weak("No accounts yet. Add one to get started.");
                    });
                    ui.add_space(8.0);
                }

                add_account = ui
                    .add_sized(
                        [ui.available_width(), 30.0],
                        egui::Button::new(format!("{} Add account", ICON_ADD.codepoint)),
                    )
                    .clicked();

                if !self.add_account_open
                    && let Some(error) = &self.account_error
                {
                    ui.add_space(12.0);
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(24.0);
            });
        });

        if add_account {
            self.account_form = AccountForm::default();
            self.account_error = None;
            self.add_account_open = true;
        }
        match action {
            Some(AccountAction::Open(account)) => self.open_account(ui.ctx(), account, false),
            Some(AccountAction::ChooseWorkspace(account)) => {
                self.open_account(ui.ctx(), account, true);
            }
            Some(AccountAction::LogOut(account)) => self.log_out_account(&account),
            None => {}
        }
        self.show_add_account(ui.ctx());
    }

    fn open_account(&mut self, ctx: &egui::Context, mut account: Account, choose_workspace: bool) {
        if choose_workspace {
            account.last_workspace_id = None;
            if let Err(error) = self.app_state.set_last_workspace(&account, None) {
                self.account_error = Some(error.to_string());
            }
        }
        self.switch_account(ctx, account);
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

    fn show_add_account(&mut self, ctx: &egui::Context) {
        if !self.add_account_open {
            return;
        }
        let mut close = false;
        let pending = self.pending_account_request.is_some();
        let ready = !pending
            && !self.account_form.email.trim().is_empty()
            && !self.account_form.password.is_empty()
            && (!self.account_form.register || !self.account_form.display_name.trim().is_empty());
        let response = egui::Modal::new(egui::Id::new("add-account")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading("Add account");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.account_form.register, false, "Log in");
                ui.selectable_value(&mut self.account_form.register, true, "Register");
            });
            ui.add_space(12.0);
            ui.label("Server");

            if platform::HAS_EMBEDDED_SERVER {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.account_form.remote, false, "Local");
                    ui.selectable_value(&mut self.account_form.remote, true, "Remote");
                });
            }
            if self.account_form.remote {
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.account_form.remote_url)
                        .hint_text("https://example.com")
                        .desired_width(f32::INFINITY),
                );
            }
            ui.add_space(12.0);
            ui.label("Email address");
            ui.add(
                egui::TextEdit::singleline(&mut self.account_form.email)
                    .hint_text("you@example.com")
                    .desired_width(f32::INFINITY),
            );
            if self.account_form.register {
                ui.add_space(12.0);
                ui.label("Display name");
                ui.add(
                    egui::TextEdit::singleline(&mut self.account_form.display_name)
                        .desired_width(f32::INFINITY),
                );
            }
            ui.add_space(12.0);
            ui.label("Password");
            ui.add(
                egui::TextEdit::singleline(&mut self.account_form.password)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
            if let Some(error) = &self.account_error {
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(16.0);
            let mut submit = false;
            egui::Sides::new().show(
                ui,
                |ui| {
                    if pending {
                        ui.spinner();
                        ui.weak("Contacting server\u{2026}");
                    }
                },
                |ui| {
                    submit = ui
                        .add_enabled(
                            ready,
                            egui::Button::new(if self.account_form.register {
                                "Register"
                            } else {
                                "Log in"
                            })
                            .selected(ready),
                        )
                        .clicked();
                    close |= ui.button("Cancel").clicked();
                },
            );
            submit
        });
        if response.inner {
            self.begin_account_request();
            return;
        }
        if close || response.should_close() {
            self.add_account_open = false;
            self.pending_account_request = None;
            self.account_error = None;
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
                self.workspace_name.clear();
                self.open_workspace(workspace);
            }
            Ok(WorkspaceResult::Responded) => {
                self.workspaces_loaded = false;
                self.workspaces_load_failed = false;
                self.begin_workspace_request(WorkspaceOperation::Load);
            }
            Ok(WorkspaceResult::Invited) => {
                self.invite_email.clear();
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
            password: String::new(),
            pending: None,
            error: None,
        });
    }

    fn begin_reauth_request(&mut self) {
        let Some(reauth) = &self.reauth else {
            return;
        };
        if reauth.pending.is_some() {
            return;
        }
        let url = match &reauth.account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        let email = reauth.account.email.clone();
        let password = reauth.password.clone();
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

    fn show_reauth(&mut self, ctx: &egui::Context) {
        self.poll_reauth_request();
        let Some(reauth) = &mut self.reauth else {
            return;
        };
        let busy = reauth.pending.is_some();
        let ready = !busy && !reauth.password.is_empty();
        let mut submit = false;
        let mut log_out = false;
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("reauth")).show(ctx, |ui| {
            ui.set_width(320.0);
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.heading("Session expired");
                },
                |ui| {
                    close = ui.button(ICON_CLOSE).on_hover_text("Close").clicked();
                },
            );
            ui.add_space(8.0);
            ui.label(format!(
                "Your session for {} is no longer valid. Enter your password to sign in again, \
                 or close this to switch accounts.",
                reauth.account.email
            ));
            ui.add_space(12.0);
            ui.label("Password");
            let response = ui.add(
                egui::TextEdit::singleline(&mut reauth.password)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
            let submitted_via_enter =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if let Some(error) = &reauth.error {
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(16.0);
            egui::Sides::new().show(
                ui,
                |ui| {
                    if busy {
                        ui.spinner();
                        ui.weak("Signing in\u{2026}");
                    }
                },
                |ui| {
                    submit = ui
                        .add_enabled(ready, egui::Button::new("Sign in").selected(ready))
                        .clicked()
                        || (ready && submitted_via_enter);
                    log_out = ui
                        .add_enabled(!busy, egui::Button::new("Log out"))
                        .clicked();
                },
            );
        });
        if submit {
            self.begin_reauth_request();
        }
        if log_out && let Some(account) = self.reauth.take().map(|reauth| reauth.account) {
            self.log_out_account(&account);
        }
        if close || response.should_close() {
            self.reauth = None;
            self.signed_in = false;
            if let Err(error) = self.app_state.clear_active_account() {
                self.account_error = Some(error.to_string());
            }
        }
    }

    fn open_workspace(&mut self, workspace: Workspace) {
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

    fn show_workspace_onboarding(&mut self, ui: &mut egui::Ui) {
        if !self.workspaces_loaded
            && !self.workspaces_load_failed
            && self.pending_workspace_request.is_none()
            && self.reauth.is_none()
        {
            self.begin_workspace_request(WorkspaceOperation::Load);
        }
        self.poll_workspace_request();
        let busy = self.pending_workspace_request.is_some();
        let can_create = !busy && !self.workspace_name.trim().is_empty();
        let mut open_workspace = None;
        let mut respond = None;
        let mut create = false;
        let mut refresh = false;
        let mut retry = false;
        let mut switch_account = false;
        let mut log_out = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            onboarding_column(ui, |ui| {
                ui.add_space(36.0);
                egui::Sides::new().shrink_left().show(
                    ui,
                    |ui| {
                        ui.heading("Workspaces");
                    },
                    |ui| {
                        ui.menu_button(ICON_MORE_HORIZ, |ui| {
                            if ui
                                .button(format!("{} Log out", ICON_LOGOUT.codepoint))
                                .clicked()
                            {
                                log_out = true;
                                ui.close();
                            }
                        })
                        .response
                        .on_hover_text("Account options");
                        refresh = ui
                            .add_enabled(!busy, egui::Button::new(ICON_REFRESH))
                            .on_hover_text("Reload workspaces")
                            .clicked();
                    },
                );
                ui.add_space(12.0);

                onboarding_card(ui, |ui| {
                    egui::Sides::new().shrink_left().show(
                        ui,
                        |ui| {
                            ui.vertical(|ui| {
                                account_name(ui, &self.account);
                                account_details(ui, &self.account);
                            });
                        },
                        |ui| {
                            switch_account = ui
                                .button(format!("{} Switch account", ICON_SWITCH_ACCOUNT.codepoint))
                                .clicked();
                        },
                    );
                });

                ui.add_space(20.0);
                ui.strong("Open a workspace");
                ui.add_space(6.0);
                for workspace in &self.workspaces {
                    if ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new(format!(
                                "{} {}",
                                ICON_WORKSPACES.codepoint, workspace.name
                            ))
                            .right_text(ICON_CHEVRON_RIGHT)
                            .truncate(),
                        )
                        .clicked()
                    {
                        open_workspace = Some(workspace.clone());
                    }
                    ui.add_space(4.0);
                }
                if self.workspaces.is_empty() {
                    onboarding_card(ui, |ui| {
                        if self.workspaces_loaded {
                            ui.weak("You do not have any workspaces yet. Create one below.");
                        } else if self.workspaces_load_failed {
                            ui.horizontal(|ui| {
                                ui.weak("Could not load workspaces.");
                                retry = ui.button("Retry").clicked();
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak("Loading workspaces\u{2026}");
                            });
                        }
                    });
                }

                if !self.invitations.is_empty() {
                    ui.add_space(20.0);
                    ui.strong(format!("{} Invitations", ICON_GROUP_ADD.codepoint));
                    ui.add_space(6.0);
                    for invitation in &self.invitations {
                        onboarding_card(ui, |ui| {
                            egui::Sides::new().shrink_left().show(
                                ui,
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(
                                                    invitation.workspace_name.as_str(),
                                                )
                                                .strong(),
                                            )
                                            .truncate(),
                                        );
                                        ui.small(format!(
                                            "Invited as {}",
                                            invitation.role.label().to_lowercase()
                                        ));
                                    });
                                },
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            !busy,
                                            egui::Button::new("Accept").selected(!busy),
                                        )
                                        .clicked()
                                    {
                                        respond = Some((invitation.id, true));
                                    }
                                    if ui
                                        .add_enabled(!busy, egui::Button::new("Decline"))
                                        .clicked()
                                    {
                                        respond = Some((invitation.id, false));
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                    }
                }

                ui.add_space(20.0);
                ui.strong("Create a workspace");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let button_width = 76.0;
                    let field_width =
                        (ui.available_width() - button_width - ui.spacing().item_spacing.x)
                            .max(80.0);
                    let response = ui.add_sized(
                        [field_width, 26.0],
                        egui::TextEdit::singleline(&mut self.workspace_name)
                            .hint_text("Workspace name"),
                    );
                    let submitted = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    let clicked = ui
                        .add_enabled_ui(can_create, |ui| {
                            ui.add_sized(
                                [button_width, 26.0],
                                egui::Button::new("Create").selected(can_create),
                            )
                            .clicked()
                        })
                        .inner;
                    create = can_create && (clicked || submitted);
                });

                if let Some(error) = &self.workspace_error {
                    ui.add_space(12.0);
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(24.0);
            });
        });

        if let Some(workspace) = open_workspace {
            self.open_workspace(workspace);
        }
        if let Some((invitation, accept)) = respond {
            self.begin_workspace_request(WorkspaceOperation::Respond(invitation, accept));
        }
        if create {
            self.begin_workspace_request(WorkspaceOperation::Create(self.workspace_name.clone()));
        }
        if refresh {
            self.workspaces_loaded = false;
            self.workspaces_load_failed = false;
            self.begin_workspace_request(WorkspaceOperation::Load);
        }
        if retry {
            self.workspaces_load_failed = false;
            self.begin_workspace_request(WorkspaceOperation::Load);
        }
        if switch_account {
            self.signed_in = false;
            if let Err(error) = self.app_state.clear_active_account() {
                self.account_error = Some(error.to_string());
            }
        }
        if log_out {
            let account = self.account.clone();
            self.log_out_account(&account);
        }
    }

    fn show_invite(&mut self, ctx: &egui::Context) {
        if !self.invite_open {
            return;
        }
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let mut open = self.invite_open;
        egui::Window::new("Invite member")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Workspace: {}", workspace.name));
                ui.label("Email address");
                ui.text_edit_singleline(&mut self.invite_email);
                ui.add_space(8.0);
                ui.label("Role");
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.invite_role,
                        WorkspaceRole::Editor,
                        WorkspaceRole::Editor.label(),
                    );
                    ui.selectable_value(
                        &mut self.invite_role,
                        WorkspaceRole::Administrator,
                        WorkspaceRole::Administrator.label(),
                    );
                });
                ui.small(match self.invite_role {
                    WorkspaceRole::Administrator => "Can open every block in the workspace.",
                    WorkspaceRole::Editor => {
                        "Can only open blocks they create or are given access to."
                    }
                });
                ui.add_space(8.0);
                if ui
                    .add_enabled(
                        !self.invite_email.trim().is_empty()
                            && self.pending_workspace_request.is_none(),
                        egui::Button::new("Send invitation"),
                    )
                    .clicked()
                {
                    self.begin_workspace_request(WorkspaceOperation::Invite(
                        workspace.id,
                        self.invite_email.clone(),
                        self.invite_role,
                    ));
                }
                if let Some(error) = &self.workspace_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        self.invite_open = open;
    }

    fn show_about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = self.about_open;
        egui::Window::new("About")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.strong("Block");
                ui.add_space(8.0);
                egui::Grid::new("about-build")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Version");
                        ui.monospace(env!("CARGO_PKG_VERSION"));
                        ui.end_row();
                        ui.label("Commit");

                        ui.add(
                            egui::Label::new(egui::RichText::new(COMMIT).monospace())
                                .selectable(true),
                        );
                        ui.end_row();
                    });
            });
        self.about_open = open;
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

    fn switch_account(&mut self, ctx: &egui::Context, account: Account) {
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
        self.client_debug_open = false;
        self.network_debug_open = false;
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
        ctx.memory_mut(|memory| *memory = Default::default());
    }

    fn intercept_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.client.network_debug_snapshot().changes_saved {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.pending_destructive_action = Some(PendingDestructiveAction::Close);
    }

    fn show_discard_confirmation(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_destructive_action.clone() else {
            return;
        };
        let mut discard = false;
        let mut cancel = false;
        let (title, message, button) = match action {
            PendingDestructiveAction::Switch(ref account) => (
                "Discard unsaved changes?",
                format!(
                    "Switching to {} will discard changes that have not reached the server.",
                    account.name
                ),
                "Discard and switch",
            ),
            PendingDestructiveAction::ChooseWorkspace => (
                "Discard unsaved changes?",
                "Switching workspaces will discard changes that have not reached the server."
                    .into(),
                "Discard and switch",
            ),
            PendingDestructiveAction::Close => (
                "Discard unsaved changes?",
                "Closing Block Editor will discard changes that have not reached the server."
                    .into(),
                "Discard and close",
            ),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(message);
                ui.horizontal(|ui| {
                    discard = ui.button(button).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if discard {
            self.pending_destructive_action = None;
            match action {
                PendingDestructiveAction::Switch(account) => {
                    self.scheduled_account_switch = Some(account);
                }
                PendingDestructiveAction::ChooseWorkspace => {
                    self.scheduled_workspace_list = true;
                }
                PendingDestructiveAction::Close => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        } else if cancel {
            self.pending_destructive_action = None;
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

    fn show_shell(&mut self, ui: &mut egui::Ui) {
        let Some(shell) = self.ensure_shell() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
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
            direct_editor_tab_ui(&mut editor, ui, &mut editors)
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
            self.handle_editor_action(ui.ctx(), action);
        }
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

    fn poll_artifacts(&mut self, ui: &mut egui::Ui) {
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
            let status = session.as_mut().map(|session| {
                session.poll(ui.ctx(), &self.registry, &self.client, &descriptor.data)
            });
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
                    ui.ctx().request_repaint();
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
        self.show_artifact_dialogs(ui);
    }

    fn show_artifact_dialogs(&mut self, ui: &mut egui::Ui) {
        if let Some(id) = self.dynamic_artifact_settings_open {
            let descriptor = self.client.dynamic_artifact(id);
            let mut session = self.dynamic_artifact_sessions.remove(&id);
            let mut draft = self.dynamic_artifact_settings.remove(&id);
            if let (Some(descriptor), Some(session)) = (&descriptor, session.as_mut()) {
                match self.show_dynamic_artifact_settings(
                    ui,
                    descriptor,
                    session.as_mut(),
                    &mut draft,
                ) {
                    ModalOutcome::Open => {}
                    ModalOutcome::Accepted(data) => {
                        self.client.set_dynamic_artifact(
                            id,
                            DynamicArtifactDescriptor {
                                source_type: descriptor.source_type,
                                data: data.clone(),
                            },
                        );
                        session.regenerate(&self.client, &data);
                        self.dynamic_artifact_errors.remove(&id);
                        draft = None;
                        self.dynamic_artifact_settings_open = None;
                    }
                    ModalOutcome::Dismissed => {
                        session.cancel_settings();
                        draft = None;
                        self.dynamic_artifact_settings_open = None;
                    }
                }
            } else {
                self.dynamic_artifact_settings_open = None;
            }
            if let Some(session) = session {
                self.dynamic_artifact_sessions.insert(id, session);
            }
            if let Some(draft) = draft {
                self.dynamic_artifact_settings.insert(id, draft);
            }
        }
        if let Some(id) = self.dynamic_artifact_unlink {
            match show_dynamic_artifact_unlink(ui.ctx()) {
                ModalOutcome::Open => {}
                ModalOutcome::Accepted(()) => {
                    self.client.clear_dynamic_artifact(id);
                    self.dynamic_artifact_errors.remove(&id);
                    self.forget_dynamic_artifact_dialogs(id);
                }
                ModalOutcome::Dismissed => self.dynamic_artifact_unlink = None,
            }
        }
    }

    fn show_dynamic_artifact_settings(
        &self,
        ui: &mut egui::Ui,
        descriptor: &DynamicArtifactDescriptor,
        session: &mut dyn ArtifactSession,
        draft: &mut Option<Vec<u8>>,
    ) -> ModalOutcome<Vec<u8>> {
        let mut outcome = ModalOutcome::Open;
        let response =
            egui::Modal::new(egui::Id::new("dynamic-artifact-settings")).show(ui.ctx(), |ui| {
                ui.set_width(320.0);
                ui.heading("Dynamic artifact settings");
                ui.add_space(12.0);
                let data = draft.get_or_insert_with(|| descriptor.data.clone());
                session.settings_ui(ui, &self.registry, &self.client, data);
                if let Some(summary) = session.summary(data) {
                    ui.add_space(12.0);
                    ui.weak(summary);
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(*data != descriptor.data, egui::Button::new("Apply"))
                        .on_disabled_hover_text("The settings are unchanged")
                        .clicked()
                    {
                        outcome = ModalOutcome::Accepted(data.clone());
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = ModalOutcome::Dismissed;
                    }
                });
            });
        if matches!(outcome, ModalOutcome::Open) && response.should_close() {
            outcome = ModalOutcome::Dismissed;
        }
        outcome
    }

    fn handle_editor_action(&mut self, context: &egui::Context, action: EditorAction) {
        match action {
            EditorAction::OpenBlock {
                id,
                block_type,
                via,
                from,
            } => self.show_in_shell(id, block_type, via, from),
            EditorAction::DragBlock { id, block_type } => {
                egui::DragAndDrop::set_payload(
                    context,
                    SidebarDragPayload {
                        block_id: id,
                        block_type,
                    },
                );
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

    fn show_rename(&mut self, ui: &mut egui::Ui) {
        let Some(rename) = &mut self.rename else {
            return;
        };
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Rename block")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                let response = ui.text_edit_singleline(&mut rename.name);
                let valid = rename.name.len() <= MAX_NAME_BYTES;
                if !valid {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Name must be at most {MAX_NAME_BYTES} UTF-8 bytes."),
                    );
                }
                ui.horizontal(|ui| {
                    submit = ui.add_enabled(valid, egui::Button::new("Rename")).clicked()
                        || (valid
                            && response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if submit {
            let rename = self.rename.take().unwrap();
            self.client.set_block_name(rename.id, rename.name);
        } else if cancel {
            self.rename = None;
        }
    }
}

enum ModalOutcome<T> {
    Open,
    Accepted(T),
    Dismissed,
}

fn show_dynamic_artifact_unlink(ctx: &egui::Context) -> ModalOutcome<()> {
    let mut outcome = ModalOutcome::Open;
    let response = egui::Modal::new(egui::Id::new("dynamic-artifact-unlink")).show(ctx, |ui| {
        ui.set_width(360.0);
        ui.heading("Unlink from the source block?");
        ui.add_space(12.0);
        ui.label("This block keeps what was generated for it, but stops being rebuilt from its source and becomes editable.");
        ui.label("The link and its settings cannot be restored.");
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Unlink").clicked() {
                outcome = ModalOutcome::Accepted(());
            }
            if ui.button("Cancel").clicked() {
                outcome = ModalOutcome::Dismissed;
            }
        });
    });
    if matches!(outcome, ModalOutcome::Open) && response.should_close() {
        outcome = ModalOutcome::Dismissed;
    }
    outcome
}

enum AccountAction {
    Open(Account),
    ChooseWorkspace(Account),
    LogOut(Account),
}

fn onboarding_column<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ONBOARDING_WIDTH.min(ui.available_width());
            let margin = ((ui.available_width() - width) / 2.0).max(0.0);
            ui.horizontal_top(|ui| {
                ui.add_space(margin);
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        add_contents(ui)
                    },
                )
                .inner
            })
            .inner
        })
        .inner
}

fn onboarding_card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

fn account_name(ui: &mut egui::Ui, account: &Account) {
    ui.add(egui::Label::new(egui::RichText::new(account.name.as_str()).strong()).truncate());
}

fn account_details(ui: &mut egui::Ui, account: &Account) {
    ui.add(egui::Label::new(egui::RichText::new(account.email.as_str()).small()).truncate());
    let server = match &account.server {
        ServerLocation::Local => format!("{} Local server", ICON_COMPUTER.codepoint),
        ServerLocation::Remote(url) => format!("{} {url}", ICON_CLOUD.codepoint),
    };
    ui.add(egui::Label::new(egui::RichText::new(server).small().weak()).truncate());
}

fn show_account_card(ui: &mut egui::Ui, account: &Account) -> Option<AccountAction> {
    let mut log_out = false;
    let mut open = false;
    let mut choose_workspace = false;
    onboarding_card(ui, |ui| {
        egui::Sides::new().shrink_left().show(
            ui,
            |ui| account_name(ui, account),
            |ui| {
                ui.menu_button(ICON_MORE_HORIZ, |ui| {
                    if ui
                        .button(format!("{} Log out", ICON_LOGOUT.codepoint))
                        .clicked()
                    {
                        log_out = true;
                        ui.close();
                    }
                })
                .response
                .on_hover_text("Account options");
            },
        );
        account_details(ui, account);
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            choose_workspace = ui
                .add(egui::Button::new(ICON_KEYBOARD_ARROW_DOWN).selected(true))
                .on_hover_text("Open a different workspace")
                .clicked();
            open = ui
                .add(egui::Button::new("Open").selected(true))
                .on_hover_text(match account.last_workspace_id {
                    Some(_) => "Open the last workspace used",
                    None => "Choose a workspace",
                })
                .clicked();
        });
    });
    if log_out {
        Some(AccountAction::LogOut(account.clone()))
    } else if choose_workspace {
        Some(AccountAction::ChooseWorkspace(account.clone()))
    } else if open {
        Some(AccountAction::Open(account.clone()))
    } else {
        None
    }
}

fn drag_source(location: BlockLocation) -> SidebarDragSource {
    match location {
        BlockLocation::Root => SidebarDragSource::Root,
        BlockLocation::Orphaned => SidebarDragSource::Orphaned,
        BlockLocation::Block(id) => SidebarDragSource::Block(Uuid::from_bytes(id)),
    }
}

impl eframe::App for BlockApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.error.is_none() {
            self.error = panic_guard::take();
        }
        if let Some(message) = self.error.clone() {
            self.show_error_window(ui, &message);
            return;
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_frame(ui, frame);
        }));
        if caught.is_err() {
            self.error =
                Some(panic_guard::take().unwrap_or_else(|| "The app stopped responding.".into()));
        }
    }

    fn on_exit(&mut self) {
        block_client::shut_down_clients();
    }
}

impl BlockApp {
    fn run_frame(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        performance::begin_frame(ui.ctx());
        plugin_host::poll(ui.ctx(), frame);
        if !self.signed_in {
            self.show_account_onboarding(ui);
            performance::end_frame();
            ui.ctx().request_repaint_after(Duration::from_millis(100));
            return;
        }
        if self.scheduled_workspace_list {
            self.scheduled_workspace_list = false;
            let mut account = self.account.clone();
            account.last_workspace_id = None;
            let _ = self.app_state.set_last_workspace(&account, None);
            self.switch_account(ui.ctx(), account);
        }
        if let Some(account) = self.scheduled_account_switch.take() {
            self.switch_account(ui.ctx(), account);
        }
        if self.workspace.is_none() {
            self.show_workspace_onboarding(ui);
            self.show_reauth(ui.ctx());
            performance::end_frame();
            ui.ctx().request_repaint_after(Duration::from_millis(100));
            return;
        }
        self.sync_ui_settings(ui.ctx());
        self.poll_workspace_request();
        self.show_reauth(ui.ctx());
        self.intercept_close(ui.ctx());
        self.process_pending_transfers();
        self.process_pending_copies();
        self.show_rename(ui);
        self.share.show(ui.ctx(), &self.client);
        self.show_client_debug(ui.ctx());
        self.show_network_debug(ui.ctx());
        debug::terminal::show(ui.ctx());
        debug::inspect::show(ui.ctx());
        debug::plugins::show(ui.ctx());
        debug::version::show(ui.ctx());
        self.show_invite(ui.ctx());
        self.show_about(ui.ctx());

        egui::Panel::bottom("app-status-bar")
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.separator();
                self.show_status_bar(ui);
            });
        self.show_shell(ui);
        self.poll_artifacts(ui);
        self.show_discard_confirmation(ui.ctx());
        performance::show(ui.ctx());
        plugin_host::flush();
        performance::end_frame();
    }

    fn sync_ui_settings(&mut self, context: &egui::Context) {
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
            self.ui_settings = Some(self.client.get_block::<UiSettings>(id));
        }
        if let Some(settings) = self.ui_settings.as_ref().and_then(BlockHandle::read) {
            context.set_zoom_factor(settings.zoom());
        }
    }

    fn show_error_window(&mut self, ui: &mut egui::Ui, message: &str) {
        let mut restart = false;
        let mut delete_client_database = false;
        #[cfg(not(target_arch = "wasm32"))]
        let mut delete_server_database = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Something went wrong");
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        ui.label(message);
                    });
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    if ui.button("Restart").clicked() {
                        restart = true;
                    }
                    if ui.button("Delete client database...").clicked() {
                        self.pending_error_action = Some(ErrorAction::DeleteClientDatabase);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if ui.button("Delete local server database...").clicked() {
                        self.pending_error_action = Some(ErrorAction::DeleteServerDatabase);
                    }
                    if ui.button("Exit").clicked() {
                        std::process::exit(1);
                    }
                });
            });
        });
        if let Some(action) = self.pending_error_action.clone() {
            let (title, confirmation) = match action {
                ErrorAction::DeleteClientDatabase => (
                    "Delete client database?",
                    "This removes every saved account on this device. You will need to sign in again.",
                ),
                #[cfg(not(target_arch = "wasm32"))]
                ErrorAction::DeleteServerDatabase => (
                    "Delete local server database?",
                    "This permanently deletes every workspace and block stored on this device's local server.",
                ),
            };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.label(confirmation);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.pending_error_action = None;
                            match action {
                                ErrorAction::DeleteClientDatabase => {
                                    delete_client_database = true;
                                }
                                #[cfg(not(target_arch = "wasm32"))]
                                ErrorAction::DeleteServerDatabase => {
                                    delete_server_database = true;
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_error_action = None;
                        }
                    });
                });
        }
        if delete_client_database {
            self.delete_client_database();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if delete_server_database {
            self.delete_server_database();
        }
        if restart {
            self.restart();
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
