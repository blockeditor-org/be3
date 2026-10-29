use reactive::Set;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod arguments;
mod compositor;
mod contacts;
mod debug;
mod desktop_host;
mod display;
mod editors;
mod frame;
mod input;
mod local_settings;
mod members;
mod plugin_host;
mod programs;
mod remap;
mod root_settings;
#[cfg(target_os = "linux")]
mod session;
mod shell_route;
mod surfaces;
mod ui;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io,
    path::PathBuf,
    rc::Rc,
};

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use accounts::{AccountError, Session};
#[cfg(target_os = "linux")]
use block_app_services::{media, notifications, tray, wifi};
use block_app_base::{app_state, host, notices, panic_guard, performance, platform};
use block_app_stack::{accounts, invite_link, keys, offline_choice};
#[cfg(not(target_arch = "wasm32"))]
use block_app_stack::embedded_server;
use app_state::{AppStateStore, SavedAccount, ServerLocation};
use be_block::{DisplaySettings, InputSettings, Root, WORKSPACE_EDITOR};
use be_graph::{Access, BlockParent};
use be_protocol::{Organization, OrganizationInvitation, OrganizationRole};
use beui::native::Document;
use block_plugin_api::{
    AccessLevel, ArtifactAction, BlockCommand, BlockLocation, HostPanel, ShellDialog,
};
use block_plugin_manifest::ShellRequest;
use editors::{
    ArtifactSession, ArtifactStatus, BlockOpen, EditorAction, EditorRegistry, PluginEditor,
    SidebarDragSource, plugin::InstanceSource,
};
use invite_link::InviteLink;
use local_settings::SettingsSync;
use root_settings::RootSettings;
use surfaces::SurfaceId;
use ui::{AccountForm, AppView, AppViewStore, ErrorAction, UiCommand};
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
fn app_id() -> &'static str {
    build_id::metadata()
        .map(|info| info.app_id.as_str())
        .unwrap_or("BlockDev")
}

pub(crate) fn build() -> &'static str {
    build_id::current().unwrap_or("no build id")
}

fn run_options() -> beui_host::native::RunOptions {
    beui_host::native::RunOptions {
        #[cfg(not(target_arch = "wasm32"))]
        app_id: Some(app_id().to_owned()),
        size: beui::vec2(1100.0, 720.0),
        ..Default::default()
    }
}

#[cfg(target_arch = "wasm32")]
const WEB_DATA_DIR: &str = "/block";

#[cfg(not(target_arch = "wasm32"))]
fn storage_dir() -> Option<PathBuf> {
    directories_next::ProjectDirs::from("", "", app_id()).map(|dirs| dirs.data_dir().to_path_buf())
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments = arguments::parse(&arguments, arguments::LINUX)?;
    #[cfg(target_os = "linux")]
    if let Some(prefix) = &arguments.install_session {
        return session::install(prefix.as_deref());
    }
    #[cfg(target_os = "linux")]
    if arguments.session {
        let console = session::start_log();
        let ran = run_app(arguments);
        if let (Err(error), Some(mut console)) = (&ran, console) {
            use std::io::Write;
            let _ = writeln!(console, "block-app: the session ended: {error}");
        }
        return ran;
    }
    run_app(arguments)
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn run_app(arguments: arguments::Arguments) -> Result<(), Box<dyn Error>> {
    panic_guard::install();
    let mut app = BlockApp::new(None).map_err(|error| error.to_string())?;
    app.session_mode = arguments.session;
    let options = run_options();
    if arguments.dev_workspace {
        app.open_dev_workspace(None);
    }
    if arguments.desktop {
        app.run_as_desktop();
    }
    run_shell(options, Shell::new(app), arguments.session)
}

#[cfg(target_os = "linux")]
fn run_shell(
    options: beui_host::native::RunOptions,
    shell: Shell,
    session: bool,
) -> Result<(), Box<dyn Error>> {
    if session {
        return beui_host::native::run_on(Box::new(beui_platform_drm::Drm), options, shell);
    }
    let open_device = beui_host::native::OpenDevice {
        extensions: &be_dmabuf::DEVICE_EXTENSIONS,
        gpu: None,
    };
    beui_host::native::run_with_device(options, open_device, shell)
}

#[cfg(windows)]
fn run_shell(
    options: beui_host::native::RunOptions,
    shell: Shell,
    _session: bool,
) -> Result<(), Box<dyn Error>> {
    let open_device = beui_host::native::OpenDevice {
        extensions: &be_win32_capture::DEVICE_EXTENSIONS,
        gpu: None,
    };
    beui_host::native::run_with_device(options, open_device, shell)
}

#[cfg(all(
    not(target_os = "linux"),
    not(windows),
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
fn run_shell(
    options: beui_host::native::RunOptions,
    shell: Shell,
    _session: bool,
) -> Result<(), Box<dyn Error>> {
    beui_host::native::run_with(options, shell)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_web(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    panic_guard::install();
    block_app_stack::discovery::load().await;
    let mut app =
        BlockApp::new().map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let options = run_options();
    let location = web_sys::window()
        .map(|window| window.location())
        .ok_or("no browser window is available")?;
    let page = web_sys::Url::new(&location.href()?)?;
    if page.search_params().has("dev-workspace") {
        app.open_dev_workspace(Some(location.origin()?));
    }
    if location.hash()?.starts_with("#invite/") {
        app.open_invite(&location.href()?);
        page.set_hash("");
        location.replace(&page.href())?;
    }
    if location.hash()?.starts_with("#contact/") {
        app.contacts.open_from_address(&location.href()?);
        page.set_hash("");
        location.replace(&page.href())?;
    }
    let mut layers: Vec<Box<dyn beui_host::native::Layer>> = Vec::new();
    if let Some(inbox) = automation()? {
        layers.push(Box::new(beui_host::native::drive::Drive::new(inbox)));
    }
    beui_host::native::run_web(&canvas_id, options, layers, Shell::new(app))
        .await
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static AUTOMATION: std::cell::RefCell<Option<beui_host::native::drive::Inbox>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn automation() -> Result<Option<beui_host::native::drive::Inbox>, wasm_bindgen::JsValue> {
    let location = web_sys::window()
        .map(|window| window.location())
        .ok_or("no browser window is available")?;
    if !web_sys::Url::new(&location.href()?)?
        .search_params()
        .has("automation")
    {
        return Ok(None);
    }
    Ok(Some(AUTOMATION.with(|automation| {
        automation.borrow_mut().get_or_insert_default().clone()
    })))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn automate(
    words: Vec<String>,
    timeout_milliseconds: i32,
) -> Result<String, wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let inbox = automation()?.ok_or("the page was not opened with ?automation")?;
    let answered = beui_host::native::drive::ask(&inbox, words);
    let canceller = answered.canceller();
    let late = wasm_bindgen::closure::Closure::once_into_js(move || {
        canceller.cancel(format!(
            "timed out after {} seconds",
            f64::from(timeout_milliseconds) / 1000.0
        ));
    });
    web_sys::window()
        .ok_or("no browser window is available")?
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            late.unchecked_ref(),
            timeout_milliseconds,
        )?;
    match answered.await {
        beui_host::native::drive::Reply::Text(text) => Ok(text),
        beui_host::native::drive::Reply::Error(error) => Err(error.into()),
        beui_host::native::drive::Reply::Image { .. } => {
            Err("the browser takes screenshots itself: use drive shot".into())
        }
        beui_host::native::drive::Reply::Painting(_) => {
            Err("the web build does not answer paint; run the app natively".into())
        }
    }
}

// unsafe(ffi)
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: beui_host::native::AndroidApp) {
    let assets = app.clone();
    block_app_stack::discovery::use_assets(move |path| {
        use std::io::Read;
        let name = std::ffi::CString::new(path)
            .map_err(|_| "the asset path is not a C string".to_owned())?;
        let mut asset = assets
            .asset_manager()
            .open(&name)
            .ok_or("no such asset")?;
        let mut bytes = Vec::new();
        asset
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    });
    panic_guard::install();
    let storage_root = app.internal_data_path();
    let options = run_options();
    let exit_code = match BlockApp::new(storage_root)
        .map_err(|error| error.to_string())
        .and_then(|block_app| {
            beui_host::native::run_with(options, Shell::new(block_app))
                .map_err(|error| error.to_string())
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
        let mut document = beui_host::native::build(|| {
            compositor::install();
            desktop_host::create();
            surfaces::create_handles();
            display::create_handle();
            let store = AppViewStore::new(AppView::default());
            view = Some(store);
            beui::reactive::view! {
                <beui::styled::Root title="Block">
                    <ui::Displays view={store} />
                </beui::styled::Root>
            }
        });
        document.on_interacted(plugin_host::start_frames);
        document.on_laid_out(|| {
            plugin_host::settle();
            compositor::notify_after_layout();
        });
        Self {
            document,
            view: view.expect("the view store is created while the document is built"),
            app,
        }
    }
}

impl beui::native::App for Shell {
    fn setup(&mut self, setup: &beui::native::Setup) {
        let waker = setup.waker.clone();
        host::install_waker(move || waker.wake());
        host::install_clock(beui::reactive::now);
        plugin_host::install(setup);
        desktop_host::start(setup);
        input::start(setup);
        #[cfg(target_os = "linux")]
        if let Some(problems) = setup.get::<beui_platform_drm::Problems>() {
            problems.listen(notices::report);
        }
        #[cfg(target_os = "linux")]
        if self.app.desktop.is_none() && session::owns_a_seat(setup) {
            self.app.desktop = Some(session::DesktopSession::start());
            session::export_environment(desktop_host::environment());
        }
        display::start(setup);
        remap::load_cached(self.app.app_state.remapper().unwrap_or_else(|error| {
            eprintln!("block-app: the input remapper kept for this session was not read: {error}");
            None
        }));
        self.app.input.boot(&self.app.app_state);
        self.app.display.boot(&self.app.app_state);
        display::boot(&self.app.app_state);
    }

    fn renderer_replaced(&mut self, setup: &beui::native::Setup) {
        plugin_host::replace_gpu(setup);
        desktop_host::replace_gpu(setup);
    }

    fn update(&mut self, context: &beui::native::Context, rect: beui::Rect) {
        let started = std::time::Instant::now();
        frame::begin(context);
        if context.hurried() {
            block_app_stack::seal_owed();
        }
        if context.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, beui::Event::Focus(true)))
        }) {
            block_app_stack::retry_now();
            self.app.retry_organizations();
        }
        desktop_host::before(context, rect, &mut self.document);
        self.app.frame(context);
        self.app.sync_ui_settings();
        if let Some(preferences) = self.app.preferences.take() {
            context.set_preference_layer(beui::PreferenceSource::App, preferences);
        }
        if let Some(scale) = self.app.scale.take() {
            context.set_zoom_factor(scale);
        }
        if let Some(share) = self.app.keyboard_share.take() {
            context.set_keyboard_share(share);
        }
        if let Some(open) = self.app.inspector_requested.take()
            && open
        {
            self.document.open_inspector();
        }
        let view = self.app.view();
        desktop_host::set_owned(
            view.screen == ui::Screen::Organization && self.app.shell.is_some(),
        );
        let store = self.view;
        compositor::notify();
        beui::native::enter(&mut self.document, || {
            display::apply_shown();
            store.set(view);
        });
        #[cfg(target_arch = "wasm32")]
        plugin_host::place_screens(
            &plugin_host::shown_blits(),
            context.pixels_per_point() / context.native_pixels_per_point(),
            beui::styled::Theme::of(self.document.preferences().theme).background,
        );
        self.document.show(context, rect);
        plugin_host::describe(context);
        #[cfg(target_os = "linux")]
        if let Some(lock) = &mut self.app.screen_lock {
            lock.note_drawn(self.document.locked());
        }
        desktop_host::after(context, &mut self.document);
        let commands = ui::take_commands();
        if !commands.is_empty() {
            for command in commands {
                self.app.command(context, command);
            }
            context.request_repaint();
        }
        frame::end(context);
        let timings = self.document.performance().latest.timings;
        performance::record_update(
            started,
            &performance::UpdateTimings {
                interaction: timings.interaction,
                layout: timings.layout,
                paint: timings.paint,
                accessibility: timings.accessibility,
            },
        );
        plugin_host::record_pacing();
    }

    fn busy(&self) -> bool {
        self.app.error.is_none()
            && (self.app.busy() || plugin_host::busy() || compositor::owes_frame())
    }

    fn environment(&self) -> Vec<(String, String)> {
        desktop_host::environment()
    }

    fn drawing(&self) -> bool {
        plugin_host::drawing()
    }

    fn request_painting(&mut self) {
        plugin_host::request_painting();
    }

    fn other_frames(&mut self) -> Vec<(String, beui::native::FrameTimes)> {
        plugin_host::take_steps()
    }

    fn close_requested(&mut self) -> bool {
        self.app.close_requested()
    }

    fn lifecycle(&mut self, change: beui::native::Lifecycle) {
        match change {
            beui::native::Lifecycle::Background => block_app_stack::backgrounded(),
            beui::native::Lifecycle::Foreground
            | beui::native::Lifecycle::Focused
            | beui::native::Lifecycle::NetworkChanged => block_app_stack::retry_now(),
        }
    }

    fn exiting(&mut self) {
        desktop_host::exiting();
        plugin_host::exiting();
        display::stop();
        #[cfg(target_os = "linux")]
        {
            self.app.media = None;
            self.app.notifications = None;
            self.app.tray = None;
            self.app.desktop = None;
        }
        block_app_stack::flush();
        block_app_stack::stop_and_wait();
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
    #[cfg(not(target_arch = "wasm32"))]
    session_mode: bool,
    account_error: Option<String>,
    organization: Option<Organization>,
    organizations: Vec<Organization>,
    joining: Option<Joining>,
    organizations_loaded: bool,
    organizations_load_failed: bool,
    organizations_cached: bool,
    be_connected: bool,
    resumed: bool,
    warmed: bool,
    plugins_in_use: Vec<String>,
    pending_organization_request:
        Option<platform::RequestResult<Result<OrganizationResult, OrganizationRequestError>>>,
    organization_created: u64,
    organization_error: Option<String>,

    reauth: Option<ReauthState>,
    invite_open: bool,
    invite_sent: u64,
    invite_link: Option<String>,
    agents_open: bool,
    agents: Vec<be_protocol::Agent>,
    agent_created: Option<accounts::AgentCredentials>,
    contacts: contacts::Contacts,
    members: members::Members,
    scheduled_organization_list: bool,
    server_url: String,
    account: Account,
    root_settings: RootSettings,
    set_up_profile: Option<Uuid>,
    choosing_session: bool,
    every_session_type: bool,
    shell: Option<Uuid>,
    windows_sent: Option<(u64, Vec<block_plugin_api::HostWindowId>)>,
    focus_reports: HashMap<Uuid, editors::FocusReport>,
    artifact_watches: HashMap<Uuid, Vec<Uuid>>,
    ui_settings: Option<(beui::PreferenceLayer, u32, u32)>,
    preferences: Option<beui::PreferenceLayer>,
    scale: Option<f32>,
    keyboard_share: Option<f32>,
    input: SettingsSync<InputSettings>,
    display: SettingsSync<DisplaySettings>,
    display_round: Option<u64>,
    block_types: HashMap<Uuid, Uuid>,
    registry: Rc<EditorRegistry>,
    editors: compositor::OwnedEditors,

    watched_artifacts: Vec<Uuid>,
    dynamic_artifact_sessions: HashMap<Uuid, Box<dyn ArtifactSession>>,
    dynamic_artifact_errors: HashMap<Uuid, String>,

    dynamic_artifact_settings: HashMap<Uuid, Vec<u8>>,

    dynamic_artifact_settings_open: Option<Uuid>,

    pending_transfers: Vec<PendingTransfer>,
    pending_copies: Vec<PendingCopy>,
    about_open: bool,
    unsent_open: bool,
    discarding_unsent: Option<Uuid>,
    programs: programs::Programs,
    app_menu: Option<ui::AppMenuView>,
    pending_destructive_action: Option<PendingDestructiveAction>,
    leaving: Option<Uuid>,
    allow_close: bool,
    data_dir: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    embedded_server: Option<embedded_server::EmbeddedServer>,
    error: Option<String>,
    pending_error_action: Option<ErrorAction>,
    inspector_requested: Option<bool>,
    dev_workspace: bool,
    keys: keys::KeyState,
    identity: Option<[u8; 32]>,
    #[cfg(target_os = "linux")]
    desktop: Option<session::DesktopSession>,
    #[cfg(target_os = "linux")]
    screen_lock: Option<session::ScreenLock>,
    #[cfg(target_os = "linux")]
    published_power: Option<block_plugin_api::PowerAvailability>,
    #[cfg(target_os = "linux")]
    media: Option<media::Media>,
    #[cfg(target_os = "linux")]
    notifications: Option<notifications::Notifications>,
    #[cfg(target_os = "linux")]
    tray: Option<tray::Tray>,
    #[cfg(target_os = "linux")]
    wifi: Option<wifi::Wifi>,
}

type Account = SavedAccount;

struct PendingAccountRequest {
    receiver: platform::RequestResult<accounts::LoginEvent>,
    browser: Option<accounts::BrowserLogin>,
    _cancel: tokio::sync::oneshot::Sender<()>,
    server: ServerLocation,
    url: String,
    authorization: Option<String>,
}

struct Joining {
    link: InviteLink,
    invitation: Option<OrganizationInvitation>,
}

enum OrganizationOperation {
    Load,
    Create(String),
    LookUp(be_keys::InviteCode),
    Join(be_keys::InviteCode),
    Invite(String, OrganizationRole),
    ListAgents(Uuid),
    CreateAgent(Uuid, String),
    RemoveAgent(Uuid, Uuid),
}

enum OrganizationResult {
    Loaded(Vec<Organization>, accounts::Keys),
    Cached(Vec<Organization>),
    Created(Organization),
    Invitation(OrganizationInvitation),
    Joined(Uuid),
    Invited(String),
    Agents(
        Option<Vec<be_protocol::Agent>>,
        Option<accounts::AgentCredentials>,
    ),
}

type OrganizationRequestError = AccountError;

struct ReauthState {
    account: Account,
    error: Option<String>,
}

#[derive(Clone)]
enum PendingDestructiveAction {
    ChooseOrganization,
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

#[cfg(not(target_arch = "wasm32"))]
const CACHES: [&str; 1] = ["plugin-cache"];

#[cfg(not(target_arch = "wasm32"))]
fn forget_caches_of_other_builds(data_dir: &Path) -> io::Result<()> {
    let stamp = data_dir.join("build-id");
    let current = build_id::current().map_err(io::Error::other)?;
    if std::fs::read_to_string(&stamp).is_ok_and(|cached| cached == current) {
        return Ok(());
    }
    for cache in CACHES {
        match std::fs::remove_dir_all(data_dir.join(cache)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    std::fs::write(stamp, current)
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
        forget_caches_of_other_builds(&data_dir)?;
        panic_guard::report_in(&data_dir);
        plugin_host::cache_in(data_dir.join("plugin-cache"));
        let embedded_server = embedded_server::start_embedded_server(data_dir.join("server"))?;
        let url = embedded_server.url.clone();
        let app_state = AppStateStore::open(data_dir.join("app.sqlite3"))?;
        let mut app = Self::with_state(app_state, url)?;
        app.programs.keep_files_in(data_dir.join("files"));
        app.data_dir = data_dir;
        app.embedded_server = Some(embedded_server);
        Ok(app)
    }

    #[cfg(target_arch = "wasm32")]
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = PathBuf::from(WEB_DATA_DIR);
        std::fs::create_dir_all(&data_dir)?;
        let app_state = AppStateStore::open(data_dir.join("app.sqlite3"))?;
        let mut app = Self::with_state(app_state, String::new())?;
        app.data_dir = data_dir;
        Ok(app)
    }

    fn with_state(
        app_state: AppStateStore,
        url: String,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let client_id = app_state.client_id()?;
        let accounts = app_state.accounts()?;
        accounts::authorize(&accounts);
        let active = app_state.active_account()?;
        let account = active
            .and_then(|(server, id)| {
                accounts
                    .iter()
                    .find(|account| account.server.key() == server && account.id == id)
            })
            .cloned();
        let signed_in = account.is_some();
        let identity = match &account {
            Some(account) => app_state.identity(account)?,
            None => None,
        };
        let account = account.unwrap_or_else(|| Account {
            server: ServerLocation::Local,
            id: Uuid::nil(),
            email: String::new(),
            name: String::new(),
            token: String::new(),
            last_organization_id: None,
            authorization: None,
        });
        let server_url = match &account.server {
            ServerLocation::Local => url.clone(),
            ServerLocation::Remote(remote) => remote.clone(),
        };
        let registry = Rc::new(EditorRegistry::new());
        let editors = compositor::OwnedEditors::install(Rc::clone(&registry), client_id);
        let mut app = Self {
            app_state,
            client_id,
            local_server_url: url,
            accounts,
            signed_in,
            add_account_open: false,
            add_account_generation: 0,
            pending_account_request: None,
            #[cfg(not(target_arch = "wasm32"))]
            session_mode: false,
            account_error: None,
            organization: None,
            organizations: Vec::new(),
            joining: None,
            organizations_loaded: false,
            organizations_load_failed: false,
            organizations_cached: false,
            be_connected: false,
            resumed: false,
            warmed: false,
            plugins_in_use: Vec::new(),
            pending_organization_request: None,
            organization_created: 0,
            organization_error: None,
            reauth: None,
            invite_open: false,
            invite_sent: 0,
            invite_link: None,
            agents_open: false,
            agents: Vec::new(),
            agent_created: None,
            contacts: contacts::Contacts::default(),
            members: members::Members::default(),
            scheduled_organization_list: false,
            server_url,
            account,
            root_settings: RootSettings::new(WORKSPACE_EDITOR),
            set_up_profile: None,
            choosing_session: false,
            every_session_type: false,
            shell: None,
            windows_sent: None,
            focus_reports: HashMap::new(),
            artifact_watches: HashMap::new(),
            ui_settings: None,
            preferences: None,
            scale: None,
            keyboard_share: None,
            input: SettingsSync::default(),
            display: SettingsSync::default(),
            display_round: None,
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
            unsent_open: false,
            discarding_unsent: None,
            programs: programs::Programs::default(),
            app_menu: None,
            pending_destructive_action: None,
            leaving: None,
            allow_close: false,
            data_dir: PathBuf::new(),
            #[cfg(not(target_arch = "wasm32"))]
            embedded_server: None,
            error: None,
            pending_error_action: None,
            inspector_requested: None,
            dev_workspace: false,
            keys: keys::KeyState::default(),
            identity,
            #[cfg(target_os = "linux")]
            desktop: None,
            #[cfg(target_os = "linux")]
            screen_lock: None,
            #[cfg(target_os = "linux")]
            published_power: None,
            #[cfg(target_os = "linux")]
            media: None,
            #[cfg(target_os = "linux")]
            notifications: None,
            #[cfg(target_os = "linux")]
            tray: None,
            #[cfg(target_os = "linux")]
            wifi: None,
        };
        app.resume_organization();
        Ok(app)
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    fn run_as_desktop(&mut self) {
        self.root_settings = RootSettings::new(be_block::DESKTOP_EDITOR);
        #[cfg(target_os = "linux")]
        {
            self.screen_lock = Some(session::ScreenLock::start());
            if self.notifications.is_none() {
                self.notifications = Some(notifications::Notifications::start());
            }
            if self.tray.is_none() {
                self.tray = Some(tray::Tray::start());
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn run_notifications(&mut self) {
        if let Some(notifications) = &mut self.notifications {
            notifications.frame();
        }
        if let Some(tray) = &mut self.tray {
            tray.frame();
        }
    }

    #[cfg(target_os = "linux")]
    fn take_power_request(&mut self) {
        let request = block_app_base::watches::take_calls(block_plugin_api::requested::power).pop();
        if let Some(action) = request
            && self
                .published_power
                .is_some_and(|power| power.allows(action))
        {
            self.request_power(action, session::Trigger::Menu);
        }
        self.run_media();
        self.run_wifi();
        display::run_epaper();
    }

    #[cfg(target_os = "linux")]
    fn request_power(&mut self, action: block_plugin_api::PowerAction, trigger: session::Trigger) {
        match action {
            block_plugin_api::PowerAction::Lock => {
                if let Some(lock) = &mut self.screen_lock {
                    lock.lock(trigger);
                }
            }
            action => {
                if let Some(desktop) = &mut self.desktop {
                    desktop.request(action);
                    host::request_repaint();
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn run_session(&mut self, context: &beui::native::Context) {
        let Some(lock) = &mut self.screen_lock else {
            return;
        };
        lock.frame();
        if desktop_host::take_lock_due() {
            lock.lock(session::Trigger::Idle);
        }
        if let Some(desktop) = &mut self.desktop {
            desktop.frame(context, lock);
        }
        let locked = lock.locked();
        desktop_host::set_locked(locked);
        remap::set_locked(locked);
        block_app_base::watches::publish::<block_plugin_api::ScreenLockedWatch>(&locked);
        if locked {
            self.app_menu = None;
        }
        let mut availability = self
            .desktop
            .as_ref()
            .map(session::DesktopSession::availability)
            .unwrap_or_default();
        availability.lock = !locked;
        if self.published_power != Some(availability) {
            self.published_power = Some(availability);
            block_app_base::watches::publish::<block_plugin_api::PowerWatch>(&availability);
        }
    }

    #[cfg(target_os = "linux")]
    fn lock_view(&self) -> ui::LockView {
        let Some(lock) = &self.screen_lock else {
            return ui::LockView::default();
        };
        let state = lock.state();
        let offered = self
            .desktop
            .as_ref()
            .map(session::DesktopSession::availability)
            .unwrap_or_default();
        ui::LockView {
            available: true,
            locked: state.locked,
            busy: state.busy,
            error: state.error,
            user: lock.user().to_owned(),
            power: [
                block_plugin_api::PowerAction::Suspend,
                block_plugin_api::PowerAction::Restart,
                block_plugin_api::PowerAction::PowerOff,
            ]
            .into_iter()
            .filter(|action| offered.allows(*action))
            .collect(),
        }
    }

    #[cfg(target_os = "linux")]
    fn run_media(&mut self) {
        let requests = block_app_base::watches::take_calls(block_plugin_api::requested::media);
        if self.media.is_none()
            && (!requests.is_empty() || block_app_base::watches::watched::<block_plugin_api::MediaWatch>())
        {
            self.media = Some(media::Media::start());
            block_app_base::watches::publish::<block_plugin_api::MediaWatch>(
                &block_plugin_api::MediaLevels::default(),
            );
        }
        let Some(media) = &mut self.media else {
            return;
        };
        for request in requests {
            media.request(request);
        }
        if let Some(levels) = media.frame() {
            block_app_base::watches::publish::<block_plugin_api::MediaWatch>(&levels);
        }
    }

    #[cfg(target_os = "linux")]
    fn run_wifi(&mut self) {
        let requests = block_app_base::watches::take_calls(block_plugin_api::requested::wifi);
        if self.wifi.is_none()
            && (!requests.is_empty() || block_app_base::watches::watched::<block_plugin_api::WifiWatch>())
        {
            self.wifi = Some(wifi::Wifi::start());
            block_app_base::watches::publish::<block_plugin_api::WifiWatch>(
                &block_plugin_api::WifiStatus::default(),
            );
        }
        let Some(wifi) = &mut self.wifi else {
            return;
        };
        for request in requests {
            wifi.request(request);
        }
        if let Some(status) = wifi.frame() {
            block_app_base::watches::publish::<block_plugin_api::WifiWatch>(&status);
        }
    }

    fn busy(&self) -> bool {
        self.error.is_none() && self.loading()
    }

    fn loading(&self) -> bool {
        self.pending_account_request
            .as_ref()
            .is_some_and(|pending| pending.browser.is_none())
            || self.pending_organization_request.is_some()
            || !self.pending_transfers.is_empty()
            || !self.pending_copies.is_empty()
            || (self.dev_workspace && self.organization.is_none())
            || block_app_stack::busy()
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
        self.add_account_open = remote_url.is_some();
        self.begin_account_request(AccountForm {
            remote: remote_url.is_some(),
            remote_url: remote_url.unwrap_or_default(),
            authorization: String::new(),
            display_name: "Dev".to_owned(),
        });
    }

    fn open_invite(&mut self, text: &str) {
        let Some(link) = InviteLink::parse(text) else {
            self.organization_error = Some(
                "That is not an invite link. Copy the whole link from the person who invited you."
                    .to_owned(),
            );
            return;
        };
        self.organization_error = None;
        self.joining = Some(Joining {
            link,
            invitation: None,
        });
        if self.signed_in && self.organizations_loaded && !self.keys.needs_recovery() {
            self.look_up_invite();
        }
    }

    fn look_up_invite(&mut self) {
        let Some(joining) = &self.joining else {
            return;
        };
        if joining.invitation.is_some() {
            return;
        }
        if !joining.link.is_for(&accounts::link_base(&self.server_url)) {
            self.organization_error = Some(format!(
                "This invite link is for {}. Switch to an account on that server to join.",
                joining.link.server.clone().unwrap_or_default()
            ));
            self.joining = None;
            return;
        }
        let code = joining.link.code.clone();
        self.begin_organization_request(OrganizationOperation::LookUp(code));
    }

    fn me(&self) -> Option<accounts::Me> {
        Some(accounts::Me {
            account: self.account.id,
            identity: self.identity?,
            display_name: self.account.name.clone(),
            email: self.account.email.clone(),
        })
    }

    fn open_any_organization(&mut self) {
        if !std::mem::take(&mut self.dev_workspace) || self.organization.is_some() {
            return;
        }
        if let Some(organization) = self.organizations.first().cloned() {
            self.open_organization(organization);
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
        let authorization = Some(form.authorization.trim().to_owned())
            .filter(|header| remote && !header.is_empty());
        be_client::authorize(&url, authorization.clone());
        let requested = url.clone();
        let display_name = form.display_name;
        let default = platform::default_account_server();
        let auth_url = default
            .filter(|server| server.api_url == url)
            .map(|server| server.auth_url)
            .unwrap_or_else(|| format!("{}/auth", accounts::link_base(&url)));
        let (sender, receiver) = host::waking_channel();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let _ = platform::spawn_request(async move {
            let notify = sender.clone();
            let result = if !remote {
                accounts::create_local_profile(requested, display_name)
                    .await
                    .map_err(|error| error.to_string())
            } else {
                accounts::browser_login(
                    requested,
                    auth_url,
                    move |browser| {
                        let _ = notify.send(accounts::LoginEvent::Browser(browser));
                    },
                    Box::pin(async {
                        let _ = cancelled.await;
                    }),
                )
                .await
            };
            let _ = sender.send(accounts::LoginEvent::Finished(result));
        });
        self.account_error = None;
        self.pending_account_request = Some(PendingAccountRequest {
            receiver,
            browser: None,
            _cancel: cancel,
            server,
            url,
            authorization,
        });
    }

    fn poll_account_request(&mut self) {
        let result = self
            .pending_account_request
            .as_ref()
            .and_then(|pending| pending.receiver.try_recv().ok());
        let Some(event) = result else {
            return;
        };
        let result = match event {
            accounts::LoginEvent::Browser(browser) => {
                #[cfg(not(target_arch = "wasm32"))]
                if !self.session_mode {
                    self.account_error = platform::open_browser(&browser.url).err();
                }
                self.pending_account_request.as_mut().unwrap().browser = Some(browser);
                return;
            }
            accounts::LoginEvent::Finished(result) => result,
        };
        let pending = self.pending_account_request.take().unwrap();
        if self.reauth.is_some() {
            self.add_account_open = false;
            self.finish_reauth_request(result);
            return;
        }
        match result {
            Ok(session) => {
                let saved = SavedAccount {
                    server: pending.server.clone(),
                    id: session.account,
                    email: session.email,
                    name: session.display_name,
                    token: session.token,
                    last_organization_id: self
                        .accounts
                        .iter()
                        .find(|account| {
                            account.server == pending.server && account.id == session.account
                        })
                        .and_then(|account| account.last_organization_id),
                    authorization: pending.authorization.clone(),
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

    fn open_account(&mut self, mut account: Account, choose_organization: bool) {
        if choose_organization {
            account.last_organization_id = None;
            if let Err(error) = self.app_state.set_last_organization(&account, None) {
                self.account_error = Some(error.to_string());
            }
        }
        self.switch_account(account);
    }

    fn log_out_account(&mut self, account: &Account) {
        if account.server == ServerLocation::Local {
            self.close_reauth();
            return;
        }
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

    fn begin_organization_request(&mut self, operation: OrganizationOperation) {
        if self.pending_organization_request.is_some() {
            return;
        }
        let url = self.server_url.clone();
        let token = self.account.token.clone();
        let me = self.me();
        let cache = self.workspace_cache();
        let account = self.account.id;
        let receiver = platform::spawn_request(async move {
            let me = || {
                me.clone().ok_or_else(|| AccountError {
                    message: "Unlock this account with its recovery phrase first.".to_owned(),
                    invalid_token: false,
                })
            };
            match operation {
                OrganizationOperation::Load => {
                    match accounts::organizations(url.clone(), token.clone()).await {
                        Ok(organizations) => {
                            if let Some(cache) = &cache {
                                let _ = cache.set_workspaces(&url, account, &organizations);
                            }
                            let keys = accounts::keys(url, token).await?;
                            Ok(OrganizationResult::Loaded(organizations, keys))
                        }
                        Err(error) if !error.invalid_token => {
                            let cached = cache
                                .and_then(|cache| cache.workspaces(&url, account).ok())
                                .unwrap_or_default();
                            match cached.is_empty() {
                                true => Err(error),
                                false => Ok(OrganizationResult::Cached(cached)),
                            }
                        }
                        Err(error) => Err(error),
                    }
                }
                OrganizationOperation::Create(name) => {
                    accounts::create_organization(url, token, name)
                        .await
                        .map(OrganizationResult::Created)
                }
                OrganizationOperation::LookUp(code) => accounts::invitation(url, token, code)
                    .await
                    .map(OrganizationResult::Invitation),
                OrganizationOperation::Join(code) => {
                    accounts::accept_invitation(url, token, me()?, code)
                        .await
                        .map(OrganizationResult::Joined)
                }
                OrganizationOperation::Invite(email, role) => block_app_stack::invite(email, role)
                    .await
                    .map(OrganizationResult::Invited)
                    .map_err(|message| AccountError {
                        message,
                        invalid_token: false,
                    }),
                OrganizationOperation::ListAgents(organization) => {
                    accounts::agents(url, token, organization)
                        .await
                        .map(|agents| OrganizationResult::Agents(Some(agents), None))
                }
                OrganizationOperation::CreateAgent(organization, name) => {
                    let created = block_app_stack::create_agent(name)
                        .await
                        .map_err(|message| AccountError {
                            message,
                            invalid_token: false,
                        })?;
                    let agents = accounts::agents(url, token, organization).await.ok();
                    Ok(OrganizationResult::Agents(agents, Some(created)))
                }
                OrganizationOperation::RemoveAgent(organization, account) => {
                    accounts::remove_agent(url.clone(), token.clone(), organization, account)
                        .await?;
                    let agents = accounts::agents(url, token, organization).await?;
                    Ok(OrganizationResult::Agents(Some(agents), None))
                }
            }
        });
        self.organization_error = None;
        self.pending_organization_request = Some(receiver);
    }

    fn poll_organization_request(&mut self) {
        let result = self
            .pending_organization_request
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(result) = result else {
            return;
        };
        self.pending_organization_request = None;
        match result {
            Ok(OrganizationResult::Cached(organizations)) => {
                self.organizations = organizations;
                self.organizations_loaded = true;
                self.organizations_load_failed = false;
                self.organizations_cached = true;
                if self.organization.is_some() || self.joining.is_some() {
                    return;
                }
                if let Some(last_organization_id) = self.account.last_organization_id
                    && let Some(organization) = self
                        .organizations
                        .iter()
                        .find(|organization| organization.id == last_organization_id)
                        .cloned()
                {
                    self.open_organization(organization);
                }
                self.open_any_organization();
            }
            Ok(OrganizationResult::Loaded(organizations, keys)) => {
                self.organizations = organizations;
                self.organizations_loaded = true;
                self.organizations_load_failed = false;
                self.organizations_cached = false;
                self.keys.loaded(keys);
                if self.keys.needs_recovery() {
                    if self.dev_workspace {
                        self.keys.skip_confirmation(
                            self.server_url.clone(),
                            self.account.token.clone(),
                            self.identity,
                        );
                    }
                    return;
                }
                if self.keys.needs_unlock(self.identity) {
                    return;
                }
                if self.joining.is_some() {
                    self.look_up_invite();
                    return;
                }
                if self.refresh_open_organization() {
                    return;
                }
                if let Some(last_organization_id) = self.account.last_organization_id
                    && let Some(organization) = self
                        .organizations
                        .iter()
                        .find(|organization| organization.id == last_organization_id)
                        .cloned()
                {
                    self.open_organization(organization);
                }
                self.open_any_organization();
            }
            Ok(OrganizationResult::Created(organization)) => {
                self.organizations.push(organization.clone());
                self.organization_created += 1;
                self.open_organization(organization);
            }
            Ok(OrganizationResult::Invitation(invitation)) => {
                if let Some(joining) = &mut self.joining {
                    joining.invitation = Some(invitation);
                }
            }
            Ok(OrganizationResult::Joined(organization)) => {
                self.joining = None;
                self.account.last_organization_id = Some(organization);
                self.organizations_loaded = false;
                self.organizations_load_failed = false;
                self.begin_organization_request(OrganizationOperation::Load);
            }
            Ok(OrganizationResult::Invited(link)) => {
                self.invite_sent += 1;
                self.invite_link = Some(link);
            }
            Ok(OrganizationResult::Agents(agents, created)) => {
                if let Some(agents) = agents {
                    self.agents = agents;
                }
                if created.is_some() {
                    self.agent_created = created;
                }
            }
            Err(error) if error.invalid_token => self.begin_reauth(),
            Err(error) => {
                self.joining = self
                    .joining
                    .take()
                    .filter(|joining| joining.invitation.is_some());
                if !self.organizations_loaded {
                    self.organizations_load_failed = true;
                }
                self.organization_error = Some(error.message);
            }
        }
    }

    fn begin_reauth(&mut self) {
        if self.reauth.is_some() {
            return;
        }
        self.reauth = Some(ReauthState {
            account: self.account.clone(),
            error: None,
        });
    }

    fn begin_reauth_request(&mut self) {
        let Some(reauth) = &self.reauth else {
            return;
        };
        let account = reauth.account.clone();
        if self.pending_account_request.is_some() {
            return;
        }
        let ServerLocation::Remote(url) = account.server else {
            self.reauth.as_mut().unwrap().error = Some(
                "This local profile is no longer available. Create a new local account.".into(),
            );
            return;
        };
        self.add_account_open = true;
        self.begin_account_request(AccountForm {
            remote: true,
            remote_url: url,
            authorization: account.authorization.unwrap_or_default(),
            display_name: String::new(),
        });
    }

    fn finish_reauth_request(&mut self, result: Result<Session, String>) {
        let Some(mut reauth) = self.reauth.take() else {
            return;
        };
        match result {
            Ok(session) => {
                if session.account != reauth.account.id {
                    let url = self.server_url.clone();
                    let token = session.token;
                    let _ = platform::spawn_request(accounts::logout(url, token));
                    reauth.error =
                        Some("Sign in to the same account to restore this session.".into());
                    self.reauth = Some(reauth);
                    return;
                }
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
                self.organization_error = None;
                self.organizations_loaded = false;
                self.organizations_load_failed = false;
                self.begin_organization_request(OrganizationOperation::Load);
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

    fn resume_organization(&mut self) {
        if !self.signed_in || self.identity.is_none() || self.organization.is_some() {
            return;
        }
        let last = self
            .app_state
            .last_organization(&self.account)
            .ok()
            .flatten()
            .filter(|organization| Some(organization.id) == self.account.last_organization_id);
        if let Some(organization) = last {
            self.open_organization(organization);
            self.resumed = true;
        }
    }

    fn refresh_open_organization(&mut self) -> bool {
        if !std::mem::take(&mut self.resumed) {
            return false;
        }
        let Some(open) = self.organization.clone() else {
            return false;
        };
        match self
            .organizations
            .iter()
            .find(|organization| organization.id == open.id)
            .cloned()
        {
            Some(listed) if listed == open => {}
            Some(listed) => {
                let _ = self
                    .app_state
                    .set_last_organization_details(&self.account, &listed);
                self.organization = Some(listed);
                self.publish_administrator();
            }
            None => {
                let mut account = self.account.clone();
                account.last_organization_id = None;
                let _ = self.app_state.set_last_organization(&account, None);
                self.switch_account(account);
            }
        }
        true
    }

    fn open_organization(&mut self, organization: Organization) {
        self.resumed = false;
        self.focus_reports.clear();
        self.artifact_watches.clear();
        block_app_stack::stop();
        self.block_types.clear();
        self.registry = Rc::new(EditorRegistry::new());
        self.editors.reset(Rc::clone(&self.registry));
        self.watched_artifacts.clear();
        self.dynamic_artifact_sessions.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.root_settings = RootSettings::new(self.root_settings.shell());
        offline_choice::forget();
        self.choosing_session = false;
        self.shell = None;
        self.input.set_layers(Default::default());
        self.display.set_layers(Default::default());
        self.keys.cancel_pairing();
        self.organization = Some(organization.clone());
        self.account.last_organization_id = Some(organization.id);
        if let Some(saved) = self
            .accounts
            .iter_mut()
            .find(|saved| saved.server == self.account.server && saved.id == self.account.id)
        {
            saved.last_organization_id = Some(organization.id);
        }
        if let Err(error) = self
            .app_state
            .set_last_organization(&self.account, Some(organization.id))
            .and_then(|()| {
                self.app_state
                    .set_last_organization_details(&self.account, &organization)
            })
        {
            self.organization_error = Some(error.to_string());
        }
    }

    fn poll_keys(&mut self) {
        let secret = match self.keys.poll() {
            Some(keys::KeyEvent::RecoverySaved(secret) | keys::KeyEvent::Unlocked(secret)) => {
                secret
            }
            None => return,
        };
        if let Err(error) = self.app_state.set_identity(&self.account, secret) {
            self.organization_error = Some(error.to_string());
            return;
        }
        self.identity = Some(secret);
        if self.organization.is_none() {
            self.organizations_loaded = false;
            self.organizations_load_failed = false;
            self.begin_organization_request(OrganizationOperation::Load);
        }
    }

    fn locked(&self) -> bool {
        self.identity.is_none() || self.keys.needs_unlock(self.identity)
    }

    fn workspace_cache(&self) -> Option<be_client::RefsDb> {
        block_app_stack::refs_in(&self.data_dir).ok()
    }

    fn retry_organizations(&mut self) {
        if self.pending_organization_request.is_some() || self.reauth.is_some() {
            return;
        }
        if self.organizations_load_failed {
            self.organizations_load_failed = false;
        } else if self.organizations_cached {
            self.begin_organization_request(OrganizationOperation::Load);
        }
    }

    fn notice_reconnect(&mut self) {
        let connected = block_app_stack::status().connected;
        if connected && !self.be_connected {
            self.retry_organizations();
        }
        self.be_connected = connected;
    }

    fn load_organizations_if_needed(&mut self) {
        if !self.organizations_loaded
            && !self.organizations_load_failed
            && self.pending_organization_request.is_none()
            && self.reauth.is_none()
        {
            self.begin_organization_request(OrganizationOperation::Load);
        }
        self.poll_organization_request();
    }

    fn switch_account(&mut self, account: Account) {
        let server_url = match &account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        self.focus_reports.clear();
        self.artifact_watches.clear();
        block_app_stack::stop();
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
        self.unsent_open = false;
        self.discarding_unsent = None;
        self.app_menu = None;
        self.pending_destructive_action = None;
        self.allow_close = false;
        self.organization = None;
        self.organizations.clear();
        self.resumed = false;
        self.organizations_loaded = false;
        self.organizations_load_failed = false;
        self.organizations_cached = false;
        self.pending_organization_request = None;
        self.organization_error = None;
        self.reauth = None;
        self.invite_open = false;
        self.invite_link = None;
        self.agents_open = false;
        self.agent_created = None;
        self.contacts.close();
        self.members.close();
        self.root_settings = RootSettings::new(self.root_settings.shell());
        offline_choice::forget();
        self.choosing_session = false;
        self.shell = None;
        self.input.set_layers(Default::default());
        self.display.set_layers(Default::default());
        self.keys = keys::KeyState::default();
        self.identity = self.app_state.identity(&account).unwrap_or_else(|error| {
            self.organization_error = Some(error.to_string());
            None
        });
        self.account = account;
        self.server_url = server_url;
        self.signed_in = true;
        if let Err(error) = self.app_state.set_active_account(&self.account) {
            self.account_error = Some(error.to_string());
        }
        self.resume_organization();
    }

    fn close_requested(&mut self) -> bool {
        if block_app_stack::flush() == 0 || self.allow_close {
            return true;
        }
        self.pending_destructive_action = Some(PendingDestructiveAction::Close);
        host::request_repaint();
        false
    }

    fn discard(&mut self, context: &beui::native::Context) {
        let Some(action) = self.pending_destructive_action.take() else {
            return;
        };
        match action {
            PendingDestructiveAction::ChooseOrganization => {
                self.scheduled_organization_list = true;
            }
            PendingDestructiveAction::Close => {
                self.allow_close = true;
                context.close_window();
            }
        }
    }

    fn block_type_of(&self, id: Uuid) -> Option<Uuid> {
        self.block_types.get(&id).copied().or_else(|| {
            block_app_stack::node(id)
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
        if !linked {
            block_app_stack::placing(child, true);
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
        block_app_stack::set_parent(id, parent);
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
        if source == SidebarDragSource::Orphaned && !is_reference {
            block_app_stack::placing(child, true);
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: Some(source),
            destination: Some(destination),
            parent_after: (!is_reference).then_some(BlockParent::Block(destination)),
            stage: TransferStage::DeleteSource,
        });
    }

    fn restore(&mut self, id: Uuid) {
        let Some(root) = block_app_stack::with_graph(|graph| graph.deleted_root(id)).flatten() else {
            return;
        };
        let Some(node) = block_app_stack::node(root) else {
            return;
        };
        let usable = |parent: Uuid| {
            block_app_stack::node(parent).is_some_and(|parent| parent.access.can_edit())
                && block_app_stack::with_graph(|graph| graph.deleted_root(parent).is_none()).unwrap_or(false)
        };
        let addable = |parent: Uuid| {
            block_app_stack::node(parent)
                .and_then(|parent| block_app_stack::discovery::block_type(parent.content_type))
                .is_some_and(|block_type| block_type.children.add)
        };
        if let Some(BlockParent::Block(parent)) = node.parent
            && usable(parent)
            && addable(parent)
        {
            self.queue_placement(root, node.content_type, parent, false);
            return;
        }
        let from = match node.parent {
            Some(BlockParent::Block(parent)) => Some(be_block::DeletedFrom::Block(parent)),
            _ => node.metadata.get(&be_block::DELETED_FROM),
        };
        let destination = match from {
            Some(be_block::DeletedFrom::Root) if addable(root) => {
                block_app_stack::set_parent(root, BlockParent::Root);
                return;
            }
            Some(be_block::DeletedFrom::Block(parent)) if usable(parent) && addable(parent) => {
                parent
            }
            Some(be_block::DeletedFrom::Root | be_block::DeletedFrom::Block(_)) | None => {
                match self.root_settings.my_files() {
                    Some(files) => files,
                    None => return,
                }
            }
        };
        let listed = block_app_stack::node(destination).is_some_and(|parent| parent.references.contains(&root));
        match listed {
            true => block_app_stack::set_parent(root, BlockParent::Block(destination)),
            false => self.queue_move(
                root,
                node.content_type,
                SidebarDragSource::Orphaned,
                destination,
                false,
            ),
        }
    }

    fn queue_delete(&mut self, child: Uuid, block_type: Uuid, source: SidebarDragSource) {
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
            parent_after: None,
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
                        block_app_stack::set_parent(transfer.child, BlockParent::Detached);
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
            if !matches!(transfer.parent_after, Some(BlockParent::Block(_))) {
                block_app_stack::placing(transfer.child, false);
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
                let Some((copy_id, block_type)) = block_app_stack::duplicate(copy.source) else {
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
                BlockOpen {
                    id: copy_id,
                    block_type,
                    via: Some(copy.container),
                    editor: None,
                },
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
        self.root_settings.ensure_my_files();
        let class = root_settings::device_class();
        self.root_settings.ensure_profile(self.client_id, class)?;
        if let Some(profile) = self.root_settings.take_set_up() {
            self.set_up_profile = Some(profile);
        }
        let id = self.root_settings.ensure_session(self.client_id, class)?;
        if self.shell.is_some_and(|previous| previous != id) {
            self.stop_shell();
        }
        let shell_editor = self.root_settings.session_editor(id)?;
        let view_type = block_app_stack::node(id)?.content_type;
        self.block_types.insert(id, view_type);
        let open_as = self
            .editors
            .with(|open| open.get(&id).and_then(PluginEditor::editor_id));
        if open_as != Some(shell_editor) {
            let opened = editors::Opened {
                id,
                block_type: view_type,
                view_block: Some(id),
            };
            let editor = self.registry.open_with(opened, Some(shell_editor));
            if let Some(missing) = ShellRequest::ROUTED
                .into_iter()
                .find(|request| !editor.accepts(*request))
            {
                eprintln!("the shell editor does not accept {missing:?} requests");
            }
            let replaced = self.editors.with(|open| open.insert(id, editor));
            drop(replaced);
        }
        self.shell = Some(id);
        Some(id)
    }

    fn stop_shell(&mut self) {
        let Some(previous) = self.shell.take() else {
            return;
        };
        let open: Vec<Uuid> = self.editors.with(|open| open.keys().copied().collect());
        for editor in open {
            self.close_editor(editor);
        }
        self.block_types.remove(&previous);
        compositor::set_shell(None);
        plugin_host::set_shell(None);
    }

    fn watch_shell(&mut self) {
        let crashed = plugin_host::take_crashed();
        let Some(shell) = self.shell else {
            return;
        };
        let plugin = self
            .with_editor(shell, |editor| {
                editor.plugin().map(|plugin| plugin.identity.id.clone())
            })
            .flatten();
        let Some(plugin) = plugin.filter(|plugin| crashed.contains(plugin)) else {
            return;
        };
        self.stop_shell();
        plugin_host::kill(&plugin);
        self.choosing_session = true;
    }

    fn handler_for(&self, from: Option<Uuid>, request: ShellRequest) -> Option<Uuid> {
        Some(shell_route::handler(
            from,
            self.shell?,
            |id| self.with_editor(id, |editor| editor.accepts(request)) == Some(true),
            |id| self.editors.parent_of(id),
        ))
    }

    fn child_within(&self, ancestor: Uuid, block: Uuid) -> Option<Uuid> {
        let mut visited = HashSet::new();
        let mut current = block;
        while visited.insert(current) {
            let parent = self.editors.parent_of(current)?;
            if parent == ancestor {
                return Some(current);
            }
            current = parent;
        }
        None
    }

    fn show_dialog(&mut self, from: Option<Uuid>, id: Uuid, dialog: ShellDialog) {
        if let Some(handler) = self.handler_for(from, ShellRequest::Dialog) {
            self.with_editor(handler, |handler| handler.show_dialog(id, dialog));
        }
    }

    fn show_rename(
        &mut self,
        from: Option<Uuid>,
        id: Uuid,
        anchor: Option<block_plugin_api::Rect>,
    ) {
        let Some(handler) = self.handler_for(from, ShellRequest::Dialog) else {
            return;
        };
        let source = from.and_then(|from| self.with_editor(from, |editor| editor.source()));
        let target = self.with_editor(handler, |editor| editor.source());
        let anchor = (|| {
            let (source, target) = (source.flatten()?, target.flatten()?);
            crate::plugin_host::reanchor(
                anchor?,
                crate::plugin_host::frame_placement(&source.plugin_id, source.instance)?,
                crate::plugin_host::frame_placement(&target.plugin_id, target.instance)?,
            )
        })();
        let within = from.and_then(|from| self.child_within(handler, from));
        self.with_editor(handler, |handler| {
            handler.show_dialog(
                id,
                ShellDialog::Rename {
                    anchor,
                    within: within.map(Uuid::into_bytes),
                },
            );
        });
    }

    fn show_panel(&mut self, panel: HostPanel) {
        if let Some(shell) = self.shell {
            self.with_editor(shell, |shell| shell.show_panel(panel));
        }
    }

    fn show_block(&mut self, from: Option<Uuid>, open: BlockOpen) {
        self.block_types.insert(open.id, open.block_type);
        if let Some(handler) = self.handler_for(from, ShellRequest::ShowBlock) {
            self.with_editor(handler, |handler| handler.show_block(open));
        }
    }

    fn close_editor(&mut self, id: Uuid) {
        if self.shell == Some(id) {
            return;
        }
        programs::close(id);
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
                let data = artifact_of(id).map(|artifact| artifact.settings);
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
        if self
            .with_editor(shell, |editor| editor.claim_shell())
            .is_none()
        {
            return;
        }
        if let Some(profile) = self.set_up_profile.take() {
            self.show_block(
                None,
                BlockOpen {
                    id: profile,
                    block_type:
                        <be_block::SettingsProfileContent as be_block::BlockContent>::CONTENT_TYPE,
                    via: None,
                    editor: None,
                },
            );
        }
        let hidden = programs::bound_windows();
        if Some((desktop_host::revision(), hidden.clone())) != self.windows_sent {
            block_app_base::watches::publish::<block_plugin_api::WindowsWatch>(&desktop_host::listed(&hidden));
            self.windows_sent = Some((desktop_host::revision(), hidden));
        }
        for action in block_app_base::watches::take_calls(block_plugin_api::requested::window) {
            match action {
                block_plugin_api::WindowAction::Focus(window) => desktop_host::focus(window),
                block_plugin_api::WindowAction::Close(window) => desktop_host::close(window),
                block_plugin_api::WindowAction::Show(window) => desktop_host::show(window),
                block_plugin_api::WindowAction::Hide(window) => desktop_host::hide(window),
                block_plugin_api::WindowAction::Fullscreen { window, fullscreen } => {
                    desktop_host::fullscreen(window, fullscreen);
                }
            }
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
        self.forward_child_commits();
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

    fn forward_child_commits(&mut self) {
        let editors: Vec<(Uuid, InstanceSource)> = self.editors.with(|open| {
            open.values()
                .filter_map(|editor| Some((editor.id(), editor.source()?)))
                .collect()
        });
        let hosted = self.editors.hosted_sources();
        for (id, source) in editors.iter().chain(&hosted) {
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
            let (settings, dismissed): (Vec<_>, Vec<_>) =
                crate::plugin_host::take_child_dismissals(&source.plugin_id, source.instance)
                    .into_iter()
                    .partition(|child| self.editors.settings_block(*id, *child).is_some());
            for child in settings {
                if let Some(session) = self
                    .editors
                    .settings_block(*id, child)
                    .and_then(|block| self.dynamic_artifact_sessions.get_mut(&block))
                {
                    session.dismiss();
                }
            }
            if !dismissed.is_empty() {
                self.editors.dismiss_creations(*id, &dismissed);
            }
        }
        self.editors.forget_hidden_settings();
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
                    *descriptor.source,
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
                .map(|session| session.poll(&self.registry, &descriptor.settings));
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
                source: descriptor.source.into_bytes(),
                summary: None,
                error: self.dynamic_artifact_errors.get(&id).cloned(),
                regenerating: session
                    .as_ref()
                    .is_some_and(|session| session.regenerating()),
            };
            match &status {
                Some(ArtifactStatus::Described(summary)) => {
                    state.summary = Some(summary.clone());
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
            .or_insert_with(|| descriptor.settings.clone());
        surfaces::host(
            SurfaceId::ArtifactSettings,
            Some(session.settings_region(&self.registry)),
        );
        if let Some(edited) = session.take_draft() {
            *draft = edited;
        }
        self.editors
            .set_settings_status(Some((id, session.settings_height(), session.dialog())));
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
            Some(be_block::Artifact {
                settings: data.clone(),
                ..descriptor
            }),
        );
        if let Some(session) = self.dynamic_artifact_sessions.get_mut(&id) {
            session.regenerate(&data);
            session.close_dialog();
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
            EditorAction::OpenBlock(open) => self.show_block(from, open),
            EditorAction::Command { id, command } => self.handle_block_command(id, command),
            EditorAction::Shell { id, command } => self.handle_shell_command(from, id, command),
            EditorAction::SimulateAccess { id, access } => self.simulate_access(id, access),
            EditorAction::AppMenu { at } => self.open_app_menu(at),
        }
    }

    fn open_app_menu(&mut self, at: Option<beui::Rect>) {
        let opened = self.app_menu.as_ref().map_or(0, |menu| menu.opened) + 1;
        self.app_menu = Some(ui::AppMenuView { opened, at });
    }

    fn handle_shell_command(
        &mut self,
        from: Option<Uuid>,
        id: Uuid,
        command: block_plugin_api::ShellCommand,
    ) {
        use block_plugin_api::ShellCommand;
        match command {
            ShellCommand::Share => self.show_dialog(from, id, ShellDialog::Share),
            ShellCommand::Leave => self.leaving = Some(id),
            ShellCommand::Rename { anchor } => self.show_rename(from, id, anchor),
            ShellCommand::CloseEditor => self.close_editor(id),
            ShellCommand::AppMenu { .. } => self.open_app_menu(None),
        }
    }

    fn simulate_access(&mut self, id: Uuid, access: AccessLevel) {
        let access = match access {
            AccessLevel::None => Access::None,
            AccessLevel::KnowExists => Access::KnowExists,
            AccessLevel::View => Access::View,
            AccessLevel::Edit => Access::Edit,
            AccessLevel::Manage => Access::Manage,
        };
        match access == Access::Manage {
            true => self
                .editors
                .with_simulated(|simulated| simulated.remove(&id)),
            false => self
                .editors
                .with_simulated(|simulated| simulated.insert(id, access)),
        };
    }

    fn handle_block_command(&mut self, id: Uuid, command: BlockCommand) {
        match command {
            BlockCommand::Undo if self.editor_access(id).can_edit() => block_app_stack::undo(id),
            BlockCommand::Redo if self.editor_access(id).can_edit() => block_app_stack::redo(id),
            BlockCommand::Undo | BlockCommand::Redo => {}
            BlockCommand::Unlink { container } => {
                self.queue_copy(id, Uuid::from_bytes(container));
            }
            BlockCommand::Artifact { action } => self.act_on_artifact(id, action),
            BlockCommand::Delete { block_type, source } => {
                self.queue_delete(id, Uuid::from_bytes(block_type), drag_source(source))
            }
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
            BlockCommand::Restore => self.restore(id),
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

    fn frame(&mut self, context: &beui::native::Context) {
        #[cfg(not(target_os = "linux"))]
        let _ = context;
        if self.error.is_none()
            && let Some(report) = panic_guard::take_previous()
        {
            self.crashed_before(report);
        }
        if self.error.is_none()
            && let Some(report) = panic_guard::take()
        {
            self.crashed(report);
        }
        if self.error.is_none() {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.run_frame();
            }));
            if caught.is_err() {
                self.crashed(
                    panic_guard::take().unwrap_or_else(|| "The app stopped responding.".into()),
                );
            }
        }
        self.run_display_prompt();
        #[cfg(target_os = "linux")]
        self.run_session(context);
    }

    fn run_display_prompt(&mut self) {
        let now = host::now();
        let mut prompt = display::prompt(now);
        if prompt == display::Prompt::Expired {
            let edits = display::revert(&self.app_state, &self.display.layers());
            self.display.commit(&self.app_state, edits);
            prompt = display::prompt(now);
        }
        self.display_round = match prompt {
            display::Prompt::Asking { round, left } => {
                host::request_repaint_after(left);
                Some(round)
            }
            display::Prompt::Settled | display::Prompt::Expired => None,
        };
    }

    fn answer_display(&mut self, round: u64, keep: bool) {
        let display::Prompt::Asking { round: asked, .. } = display::prompt(host::now()) else {
            return;
        };
        if asked != round {
            return;
        }
        let edits = match keep {
            true => display::keep(&self.app_state, &self.display.layers()),
            false => display::revert(&self.app_state, &self.display.layers()),
        };
        self.display.commit(&self.app_state, edits);
        self.display_round = None;
    }

    fn crashed(&mut self, report: String) {
        let _ = self.app_state.clear_active_account();
        #[cfg(target_os = "linux")]
        if self.desktop.is_some() {
            panic_guard::persist(&report);
            std::panic::resume_unwind(Box::new(panic_guard::summary(&report).to_owned()));
        }
        panic_guard::forget();
        self.error = Some(report);
    }

    fn crashed_before(&mut self, report: String) {
        #[cfg(target_os = "linux")]
        if self.desktop.is_some() {
            let summary = panic_guard::summary(&report);
            let message = match summary.lines().skip(1).collect::<Vec<_>>().join(" ") {
                message if message.is_empty() => summary.to_owned(),
                message => message,
            };
            notices::report_with(
                format!("The last session ended after an error: {message}"),
                report,
            );
            return;
        }
        self.crashed(format!(
            "Block closed after this error the last time it ran.\n\n{report}"
        ));
    }

    fn run_frame(&mut self) {
        performance::begin_frame();
        plugin_host::poll();
        self.watch_shell();
        self.poll_account_request();
        #[cfg(target_os = "linux")]
        self.run_notifications();
        if !self.signed_in {
            block_app_stack::stop();
            performance::end_frame();
            return;
        }
        self.warm_plugins();
        if self.scheduled_organization_list {
            self.scheduled_organization_list = false;
            let mut account = self.account.clone();
            account.last_organization_id = None;
            let _ = self.app_state.set_last_organization(&account, None);
            self.switch_account(account);
        }
        self.poll_keys();
        if self.organization.is_none() {
            block_app_stack::stop();
            self.load_organizations_if_needed();
            performance::end_frame();
            return;
        }
        if self.locked() {
            block_app_stack::stop();
            if !self.organizations_loaded {
                self.load_organizations_if_needed();
            }
            self.poll_organization_request();
            performance::end_frame();
            return;
        }
        self.sync_local_settings();
        self.programs.frame();
        self.sync_be_stack();
        self.notice_reconnect();
        let ready = self
            .organization
            .as_ref()
            .is_some_and(|organization| block_app_stack::installed_for(self.account.id, organization.id));
        self.contacts
            .poll(&accounts::link_base(&self.server_url), ready);
        self.contacts.publish();
        self.members.poll();
        self.load_organizations_if_needed();
        if self.choosing_session {
            self.root_settings.ensure();
            performance::end_frame();
            return;
        }
        self.process_pending_transfers();
        self.process_pending_copies();
        self.show_shell();
        #[cfg(windows)]
        desktop_host::frame();
        #[cfg(target_os = "linux")]
        {
            self.take_power_request();
            remap::frame(&self.app_state);
        }
        self.poll_artifacts();
        self.record_plugins_in_use();
        plugin_host::flush();
        performance::end_frame();
    }

    fn warm_plugins(&mut self) {
        if std::mem::replace(&mut self.warmed, true) {
            return;
        }
        let shell = self.root_settings.shell();
        let in_use = self.app_state.warm_plugins().unwrap_or_default();
        for manifest in block_app_stack::discovery::manifests() {
            let edits_shell = manifest
                .editors
                .iter()
                .any(|editor| Uuid::from_bytes(editor.id) == shell);
            if edits_shell || in_use.contains(&manifest.identity.id) {
                plugin_host::warm(&manifest);
            }
        }
        self.plugins_in_use = in_use;
    }

    fn record_plugins_in_use(&mut self) {
        let in_use = plugin_host::in_use();
        if in_use.is_empty() || in_use == self.plugins_in_use {
            return;
        }
        if let Err(error) = self.app_state.set_warm_plugins(&in_use) {
            eprintln!("block-app: the plugins in use were not saved: {error}");
        }
        self.plugins_in_use = in_use;
    }

    fn sync_be_stack(&mut self) {
        let Some(organization) = self
            .organization
            .as_ref()
            .map(|organization| organization.id)
        else {
            return;
        };
        let Some(identity) = self.identity else {
            return;
        };
        if block_app_stack::installed_for(self.account.id, organization) {
            return;
        }
        let link_base = if self.server_url == self.local_server_url {
            String::new()
        } else {
            accounts::link_base(&self.server_url)
        };
        block_app_base::watches::publish::<block_plugin_api::LinkBaseWatch>(&link_base);
        self.publish_administrator();
        block_app_stack::start(block_app_stack::Config {
            server_url: self.server_url.clone(),
            token: self.account.token.clone(),
            account: self.account.id,
            organization,
            identity,
            display_name: self.account.name.clone(),
            email: self.account.email.clone(),
            data_dir: self.data_dir.clone(),
            clock: be_client::Clock::real(),
        });
    }

    fn publish_administrator(&self) {
        block_app_base::watches::publish::<block_plugin_api::AdministratorWatch>(
            &self.organization.as_ref().is_some_and(|organization| {
                organization.kind == be_protocol::OrganizationKind::Company
                    && organization.role == OrganizationRole::Administrator
            }),
        );
    }

    fn sync_local_settings(&mut self) {
        if self.root_settings.loaded() {
            let settings = &self.root_settings;
            let company = self
                .organization
                .as_ref()
                .filter(|organization| organization.kind == be_protocol::OrganizationKind::Company)
                .map(|organization| organization.id);
            offline_choice::publish(
                &settings.offline_settings(self.client_id, company),
                settings.workspaces(),
                company.is_none(),
            );
            self.input
                .set_layers(settings.layers(InputSettings::CONTENT_TYPE, self.client_id));
            self.display
                .set_layers(settings.layers(DisplaySettings::CONTENT_TYPE, self.client_id));
        }
        self.input.sync(&self.app_state);
        self.display.sync(&self.app_state);
    }

    fn sync_ui_settings(&mut self) {
        let Some(settings) = self.display.applied() else {
            return;
        };
        let shown = (
            settings.preferences(),
            settings.scale_percent(),
            settings.keyboard_percent(),
        );
        if self.ui_settings == Some(shown) {
            return;
        }
        self.ui_settings = Some(shown);
        self.preferences = Some(shown.0);
        self.scale = Some(settings.scale());
        self.keyboard_share = Some(settings.keyboard_share());
    }

    fn command(&mut self, context: &beui::native::Context, command: UiCommand) {
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
            UiCommand::CloseApp => {
                if self.close_requested() {
                    context.close_window();
                }
            }
            UiCommand::OpenAccount(key) => {
                if let Some(account) = self.account_by_key(&key) {
                    self.open_account(account, false);
                }
            }
            UiCommand::ChooseOrganization(key) => {
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
            UiCommand::OpenLoginBrowser => {
                if let Some(browser) = self
                    .pending_account_request
                    .as_ref()
                    .and_then(|pending| pending.browser.as_ref())
                {
                    self.account_error = platform::open_browser(&browser.url).err();
                }
            }
            UiCommand::RetryAccount => {
                self.pending_account_request = None;
                self.account_error = None;
                if self.reauth.is_some() {
                    self.begin_reauth_request();
                }
            }
            UiCommand::CloseAddAccount => {
                self.add_account_open = false;
                self.pending_account_request = None;
                self.account_error = None;
            }
            UiCommand::ReloadOrganizations => {
                self.organizations_loaded = false;
                self.organizations_load_failed = false;
                self.begin_organization_request(OrganizationOperation::Load);
            }
            UiCommand::OpenOrganization(id) => {
                if let Some(organization) = self
                    .organizations
                    .iter()
                    .find(|organization| organization.id == id)
                    .cloned()
                {
                    self.open_organization(organization);
                }
            }
            UiCommand::ChooseSession(id) => {
                if let Some(organization) = self
                    .organizations
                    .iter()
                    .find(|organization| organization.id == id)
                    .cloned()
                {
                    self.open_organization(organization);
                    self.choosing_session = true;
                }
            }
            UiCommand::SwitchSession => self.choosing_session = true,
            UiCommand::CloseSessions => self.choosing_session = false,
            UiCommand::OpenSession(session) => {
                self.root_settings.use_session(self.client_id, session);
                self.choosing_session = false;
            }
            UiCommand::OpenNewSession(editor) => {
                self.root_settings.new_session(self.client_id, editor);
                self.choosing_session = false;
            }
            UiCommand::EverySessionType(every) => self.every_session_type = every,
            UiCommand::UseProfile(profile) => {
                self.root_settings.use_profile(self.client_id, profile);
            }
            UiCommand::NewProfile => self.root_settings.new_profile(self.client_id),
            UiCommand::OpenInvite(text) => self.open_invite(&text),
            UiCommand::JoinInvited => {
                if let Some(joining) = &self.joining
                    && joining.invitation.is_some()
                {
                    let code = joining.link.code.clone();
                    self.begin_organization_request(OrganizationOperation::Join(code));
                }
            }
            UiCommand::CancelJoin => self.joining = None,
            UiCommand::CreateOrganization(name) => {
                if !name.trim().is_empty() {
                    self.begin_organization_request(OrganizationOperation::Create(name));
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
            UiCommand::ReauthSubmit => self.begin_reauth_request(),
            UiCommand::ReauthLogOut => {
                if let Some(account) = self.reauth.take().map(|reauth| reauth.account) {
                    self.log_out_account(&account);
                }
                self.close_reauth();
            }
            UiCommand::ReauthClose => self.close_reauth(),
            UiCommand::OpenInspector => self.inspector_requested = Some(true),
            UiCommand::InviteMember => {
                self.invite_open = true;
                self.invite_link = None;
            }
            UiCommand::SwitchOrganization => {
                if block_app_stack::flush() == 0 {
                    self.scheduled_organization_list = true;
                } else {
                    self.pending_destructive_action =
                        Some(PendingDestructiveAction::ChooseOrganization);
                }
            }
            UiCommand::About(open) => self.about_open = open,
            UiCommand::Unsent(open) => {
                self.unsent_open = open;
                self.discarding_unsent = None;
            }
            UiCommand::RestoreUnsent(block) => block_app_stack::restore_unsent(block),
            UiCommand::DiscardUnsent(block) => self.discarding_unsent = Some(block),
            UiCommand::ConfirmDiscardUnsent => {
                if let Some(block) = self.discarding_unsent.take() {
                    block_app_stack::discard_unsent(block);
                }
            }
            UiCommand::CancelDiscardUnsent => self.discarding_unsent = None,
            UiCommand::AppMenu(false) => self.app_menu = None,
            UiCommand::AppMenu(true) => self.open_app_menu(None),
            UiCommand::DismissToast(id) => notices::dismiss(id),
            UiCommand::KeepDisplay(round) => self.answer_display(round, true),
            UiCommand::RevertDisplay(round) => self.answer_display(round, false),
            #[cfg(target_os = "linux")]
            UiCommand::LockScreen => {
                if let Some(lock) = &mut self.screen_lock {
                    lock.lock(session::Trigger::Shortcut);
                }
            }
            #[cfg(target_os = "linux")]
            UiCommand::Unlock(password) => {
                if let Some(lock) = &mut self.screen_lock {
                    lock.submit(password);
                }
            }
            #[cfg(target_os = "linux")]
            UiCommand::LockPower(action) => {
                if self
                    .screen_lock
                    .as_ref()
                    .is_some_and(session::ScreenLock::locked)
                {
                    self.request_power(action, session::Trigger::Menu);
                }
            }
            #[cfg(not(target_os = "linux"))]
            UiCommand::LockScreen | UiCommand::Unlock(_) | UiCommand::LockPower(_) => {}
            UiCommand::SendInvite(email, role) => {
                if self.organization.is_some() && !email.trim().is_empty() {
                    self.invite_link = None;
                    self.begin_organization_request(OrganizationOperation::Invite(email, role));
                }
            }
            UiCommand::CloseInvite => {
                self.invite_open = false;
                self.invite_link = None;
            }
            UiCommand::OpenAgents => {
                self.agents_open = true;
                self.agent_created = None;
                if let Some(organization) = &self.organization {
                    let id = organization.id;
                    self.begin_organization_request(OrganizationOperation::ListAgents(id));
                }
            }
            UiCommand::OpenContacts => self.contacts.show(),
            UiCommand::OpenMembers => self.members.show(),
            UiCommand::CloseMembers => self.members.close(),
            UiCommand::SetMemberRole(account, role) => self.members.set_role(account, role),
            UiCommand::RemoveMember(account) => self.members.ask_remove(account),
            UiCommand::ConfirmRemoveMember => self.members.confirm_remove(),
            UiCommand::CancelRemoveMember => self.members.cancel_remove(),
            UiCommand::CloseContacts => self.contacts.close(),
            UiCommand::NewContactLink => self.contacts.new_link(),
            UiCommand::OpenContactLink(text) => self
                .contacts
                .paste(&text, &accounts::link_base(&self.server_url)),
            UiCommand::AcceptContact => self.contacts.accept(),
            UiCommand::DeclineContact => self.contacts.decline(),
            UiCommand::RemoveContact(account) => self.contacts.ask_remove(account),
            UiCommand::ConfirmRemoveContact => self.contacts.confirm_remove(),
            UiCommand::CancelRemoveContact => self.contacts.cancel_remove(),
            UiCommand::CloseAgents => {
                self.agents_open = false;
                self.agent_created = None;
            }
            UiCommand::CreateAgent(name) => {
                if let Some(organization) = &self.organization
                    && !name.trim().is_empty()
                {
                    let id = organization.id;
                    self.begin_organization_request(OrganizationOperation::CreateAgent(id, name));
                }
            }
            UiCommand::RemoveAgent(account) => {
                if let Some(organization) = &self.organization {
                    let id = organization.id;
                    self.begin_organization_request(OrganizationOperation::RemoveAgent(
                        id, account,
                    ));
                }
            }
            UiCommand::Discard => self.discard(context),
            UiCommand::CancelDiscard => self.pending_destructive_action = None,
            UiCommand::ConfirmLeave => {
                if let Some(block) = self.leaving.take() {
                    block_app_stack::leave(block);
                }
            }
            UiCommand::CancelLeave => self.leaving = None,
            UiCommand::Debug(command) => debug::command(command),
            UiCommand::ShowPanel(panel) => self.show_panel(panel),
            UiCommand::ConfirmRecovery(words) => {
                self.keys.confirm_recovery(
                    &words,
                    self.server_url.clone(),
                    self.account.token.clone(),
                    self.identity,
                );
            }
            UiCommand::UnlockWithPhrase(phrase) => {
                if let Some(secret) = self.keys.unlock_with_phrase(&phrase) {
                    if let Err(error) = self.app_state.set_identity(&self.account, secret) {
                        self.organization_error = Some(error.to_string());
                    }
                    self.identity = Some(secret);
                    if self.organization.is_none() {
                        self.begin_organization_request(OrganizationOperation::Load);
                    }
                }
            }
            UiCommand::StartPairing => {
                self.keys.start_pairing(
                    self.server_url.clone(),
                    self.account.token.clone(),
                    self.account.id,
                );
            }
            UiCommand::CancelPairing => self.keys.cancel_pairing(),
            UiCommand::NewRecoveryPhrase => self.keys.replace_recovery(),
            UiCommand::CancelRecovery => self.keys.cancel_replacing(),
            UiCommand::ApprovePairing(from, code) => block_app_stack::approve_pairing(from, code),
            UiCommand::DismissPairing(from) => block_app_stack::dismiss_pairing(from),
        }
    }

    fn view(&self) -> AppView {
        let screen = match (&self.error, self.signed_in, &self.organization) {
            (Some(_), _, _) => ui::Screen::Error,
            (None, false, _) => ui::Screen::Accounts,
            (None, true, _) if self.keys.replacing() => ui::Screen::Recovery,
            (None, true, _) if self.keys.needs_recovery() => ui::Screen::Recovery,
            (None, true, _) if self.keys.needs_unlock(self.identity) => ui::Screen::Unlock,
            (None, true, None) => ui::Screen::Organizations,
            (None, true, Some(_)) if self.identity.is_none() => ui::Screen::Unlock,
            (None, true, Some(_)) if self.choosing_session => ui::Screen::Sessions,
            (None, true, Some(_)) => ui::Screen::Organization,
        };
        let be_status = block_app_stack::status();
        let sync = match screen {
            ui::Screen::Organization => sync_state(&be_status),
            _ => ui::SyncState::Saved,
        };
        let listed = matches!(screen, ui::Screen::Accounts | ui::Screen::Organizations);
        let cache = listed.then(|| self.workspace_cache()).flatten();
        let companies = |account: &SavedAccount| -> Vec<String> {
            let Some(cache) = &cache else {
                return Vec::new();
            };
            let url = match &account.server {
                ServerLocation::Local => self.local_server_url.clone(),
                ServerLocation::Remote(url) => url.clone(),
            };
            cache
                .workspaces(&url, account.id)
                .unwrap_or_default()
                .into_iter()
                .filter(|organization| organization.kind == be_protocol::OrganizationKind::Company)
                .map(|organization| organization.name)
                .collect()
        };
        let accounts: Vec<_> = self
            .accounts
            .iter()
            .map(|account| {
                ui::account_row(
                    account,
                    account.server == self.account.server && account.id == self.account.id,
                    companies(account),
                )
            })
            .collect();
        let organization_name = self
            .organization
            .as_ref()
            .map(|organization| organization.name.clone())
            .unwrap_or_default();
        AppView {
            screen,
            error: ui::ErrorView {
                message: self.error.clone().unwrap_or_default(),
                pending: self.pending_error_action,
                unsaved: be_status.unsaved + be_status.kept,
            },
            accounts,
            account_error: self.account_error.clone(),
            add_account: ui::AddAccountView {
                open: self.add_account_open,
                generation: self.add_account_generation,
                pending: self.pending_account_request.is_some(),
                browser: self
                    .pending_account_request
                    .as_ref()
                    .and_then(|pending| pending.browser.clone()),
                error: self
                    .add_account_open
                    .then(|| self.account_error.clone())
                    .flatten(),
            },
            account: ui::account_row(&self.account, true, companies(&self.account)),
            recovery: self.keys.recovery_view(),
            unlock: self.keys.unlock_view(&self.account.name),
            pairing: match screen {
                ui::Screen::Organization => block_app_stack::pairing_requests()
                    .into_iter()
                    .map(|request| ui::PairingRow {
                        from: request.from,
                        device: request.device,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            organizations: ui::OrganizationsView {
                state: match (self.organizations_loaded, self.organizations_load_failed) {
                    (true, _) => ui::OrganizationsState::Loaded,
                    (false, true) => ui::OrganizationsState::Failed,
                    (false, false) => ui::OrganizationsState::Loading,
                },
                organizations: self
                    .organizations
                    .iter()
                    .map(|organization| (organization.id, organization.name.clone()))
                    .collect(),
                joining: self
                    .joining
                    .as_ref()
                    .and_then(|joining| joining.invitation.as_ref())
                    .map(|invitation| ui::JoiningRow {
                        organization: invitation.organization_name.clone(),
                        role: invitation.role.label().to_lowercase(),
                        email: invitation.email.clone(),
                    }),
                busy: self.pending_organization_request.is_some(),
                error: self.organization_error.clone(),
                created: self.organization_created,
            },
            reauth: self.reauth.as_ref().map(|reauth| ui::ReauthView {
                name: reauth.account.name.clone(),
                busy: self.pending_account_request.is_some(),
                error: reauth.error.clone(),
            }),
            status: ui::StatusView {
                sync,
                unsaved: be_status.unsaved > 0,
                unsent: be_status.unsent,
                saves: be_status.saves,
                organization: organization_name.clone(),
                signed_in_as: format!("Signed in as {}", self.account.name),
                sessions: self
                    .root_settings
                    .sessions(self.client_id, self.every_session_type)
                    .into_iter()
                    .map(|session| ui::SessionRow {
                        id: session.id,
                        name: session.name,
                        kind: (session.editor != self.root_settings.shell())
                            .then(|| root_settings::session_type_name(session.editor).to_owned()),
                        current: session.current,
                    })
                    .collect(),
                profiles: self
                    .root_settings
                    .profiles(self.client_id)
                    .into_iter()
                    .map(|profile| ui::ProfileRow {
                        id: profile.id,
                        name: profile.name,
                        current: profile.current,
                    })
                    .collect(),
                settings_loaded: self.root_settings.loaded(),
                every_session_type: self.every_session_type,
                session_type: self.root_settings.shell(),
                can_close: !cfg!(target_arch = "wasm32"),
                company: self.organization.as_ref().is_some_and(|organization| {
                    organization.kind == be_protocol::OrganizationKind::Company
                }),
                administrator: self.organization.as_ref().is_some_and(|organization| {
                    organization.kind == be_protocol::OrganizationKind::Company
                        && organization.role == OrganizationRole::Administrator
                }),
                escrow_warning: block_app_stack::escrow_warning().map(str::to_owned),
            },
            agents: self.agents_open.then(|| ui::AgentsView {
                organization: organization_name.clone(),
                busy: self.pending_organization_request.is_some(),
                error: self.organization_error.clone(),
                agents: self
                    .agents
                    .iter()
                    .map(|agent| ui::AgentRow {
                        account: agent.account,
                        name: agent.name.clone(),
                    })
                    .collect(),
                created: self.agent_created.as_ref().map(|created| ui::CreatedAgent {
                    name: created.name.clone(),
                    server: created.server.clone(),
                    organization: created.organization.to_string(),
                    token: created.token.clone(),
                    phrase: created.phrase.clone(),
                }),
            }),
            contacts: self.contacts.view(),
            members: self.members.view(self.account.id),
            invite: self.invite_open.then(|| ui::InviteView {
                organization: organization_name,
                busy: self.pending_organization_request.is_some(),
                error: self.organization_error.clone(),
                sent: self.invite_sent,
                link: self.invite_link.clone(),
            }),
            about: self.about_open,
            unsent: self.unsent_open.then(|| ui::UnsentView {
                rows: block_app_stack::unsent()
                    .into_iter()
                    .map(|unsent| ui::UnsentRow {
                        block: unsent.block,
                        name: unsent.name.unwrap_or_else(|| "Untitled".to_owned()),
                        offerable: unsent.offerable,
                    })
                    .collect(),
                discarding: self.discarding_unsent,
            }),
            app_menu: self.app_menu.clone(),
            discard: self.pending_destructive_action.as_ref().map(discard_view),
            leaving: self.leaving.is_some(),
            presenting: self
                .editors
                .with(|open| open.values().any(PluginEditor::presenting_now)),
            debug: debug::view(),
            toasts: ui::notice_toasts(),
            keep_display: self.display_round,
            #[cfg(target_os = "linux")]
            lock: self.lock_view(),
            #[cfg(not(target_os = "linux"))]
            lock: ui::LockView::default(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn restart(&mut self) {
        block_app_stack::flush();
        block_app_stack::stop();
        match Self::new(Some(self.data_dir.clone())) {
            Ok(mut fresh) => {
                fresh.root_settings = RootSettings::new(self.root_settings.shell());
                #[cfg(target_os = "linux")]
                {
                    fresh.desktop = self.desktop.take();
                    fresh.screen_lock = self.screen_lock.take();
                    fresh.media = self.media.take();
                    fresh.wifi = self.wifi.take();
                    fresh.notifications = self.notifications.take();
                }
                *self = fresh;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn restart(&mut self) {
        block_app_stack::flush();
        block_app_stack::stop();
        match Self::new() {
            Ok(fresh) => *self = fresh,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

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

    #[cfg(not(target_arch = "wasm32"))]
    fn delete_server_database(&mut self) {
        block_app_stack::stop_and_wait();
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

fn sync_state(status: &block_app_stack::Status) -> ui::SyncState {
    match (&status.error, status.connected) {
        (_, false) if status.connecting => ui::SyncState::Connecting,
        (_, false) => ui::SyncState::Offline {
            waiting: status.waiting,
        },
        (Some(error), true) => ui::SyncState::Error(error.clone()),
        (None, true) if status.syncing || status.waiting > 0 => ui::SyncState::Syncing,
        (None, true) => ui::SyncState::Saved,
    }
}

fn discard_view(action: &PendingDestructiveAction) -> ui::DiscardView {
    let (message, button) = match action {
        PendingDestructiveAction::ChooseOrganization => (
            "Switching organizations will discard changes that are not saved on this device yet. Changes waiting to sync are kept."
                .to_owned(),
            "Discard and switch",
        ),
        PendingDestructiveAction::Close => (
            "Closing Block Editor will discard changes that are not saved on this device yet. Changes waiting to sync are kept."
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

fn artifact_of(id: Uuid) -> Option<be_block::Artifact> {
    block_app_stack::node(id)?.metadata.get(&be_block::ARTIFACT)
}

fn set_artifact(id: Uuid, artifact: Option<be_block::Artifact>) {
    block_app_stack::define(id, vec![be_block::ARTIFACT.definition(artifact.as_ref())]);
}

fn drag_source(location: BlockLocation) -> SidebarDragSource {
    match location {
        BlockLocation::Root => SidebarDragSource::Root,
        BlockLocation::Detached => SidebarDragSource::Orphaned,
        BlockLocation::Block(id) => SidebarDragSource::Block(Uuid::from_bytes(id)),
    }
}

#[cfg(test)]
mod tests;
