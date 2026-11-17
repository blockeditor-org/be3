use beui_reactive::Set;
#[cfg(target_os = "android")]
mod android_push;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod arguments;
mod compositor;
mod contacts;
mod debug;
mod display;
mod editors;
mod frame;
mod host_files;
mod input;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod instance;
mod invitations;
mod local_settings;
#[cfg(target_os = "linux")]
mod login_session;
mod members;
#[cfg(target_os = "linux")]
mod memory;
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
mod pressed_notifications;
mod qr;
mod reveals;
mod root_settings;
mod shell_route;
mod surfaces;
mod ui;
#[cfg(target_arch = "wasm32")]
mod web_push;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    path::PathBuf,
    rc::Rc,
};

#[cfg(not(target_arch = "wasm32"))]
use std::io;

use accounts::{AccountError, Session};
use be_block::{DisplaySettings, WORKSPACE_EDITOR};
use be_block_content::Root;
use be_block_graph::{Access, BlockParent};
use be_block_input::InputSettings;
use be_block_protocol::{Organization, OrganizationRole};
use be_host_base::{host, host_state, notices, panic_guard, performance, platform};
use be_host_desktop::desktop_host::{DesktopHost, Host};
use be_host_runtime::{BlockOpen, EditorAction};
#[cfg(target_os = "linux")]
use be_host_services::{media, notifications, screenshot, tray, wifi};
use be_host_stack::{accounts, invite_link, keys, offline_choice};
use be_sdk_api::{
    AccessLevel, ArtifactAction, BlockCommand, BlockLocation, HostPanel, ShellDialog,
};
use be_sdk_manifest::ShellRequest;
use beui::native::Document;
use editors::{
    AppEditor, ArtifactSession, ArtifactStatus, EditorRegistry, SidebarDragSource,
    app::InstanceSource,
};
use host_state::{HostStateStore, SavedAccount, ServerLocation};
use invite_link::InviteLink;
use local_settings::SettingsSync;
use root_settings::RootSettings;
use surfaces::SurfaceId;
use ui::{AccountForm, ErrorAction, HostUi, HostUiStore, UiCommand};
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
fn program_id() -> &'static str {
    be_lib_build_id::metadata()
        .map(|info| info.program_id.as_str())
        .unwrap_or("be-host-dev")
}

pub(crate) fn build() -> &'static str {
    be_lib_build_id::current().unwrap_or("no build id")
}

fn run_options() -> beui_host::native::RunOptions {
    beui_host::native::RunOptions {
        #[cfg(not(target_arch = "wasm32"))]
        program_id: Some(program_id().to_owned()),
        size: beui::vec2(1100.0, 720.0),
        ..Default::default()
    }
}

#[cfg(target_arch = "wasm32")]
const WEB_DATA_DIR: &str = "/be-host";
/// The variable holding the token `--dev-workspace=URL` signs in with
/// (`HostCore::open_dev_workspace`); `main` takes it out of the environment as it starts.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub const DEV_TOKEN: &str = "BE_DEV_TOKEN";

#[cfg(not(target_arch = "wasm32"))]
fn storage_dir() -> Option<PathBuf> {
    directories_next::ProjectDirs::from("", "", program_id())
        .map(|dirs| dirs.data_dir().to_path_buf())
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run(dev_token: Option<String>) -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let platform = std::env::var(beui_host::native::environment::PLATFORM).ok();
    let arguments = arguments::parse(&arguments, platform.as_deref(), arguments::LINUX)?;
    #[cfg(target_os = "linux")]
    if let Some(setup) = &arguments.login_session {
        return match setup {
            arguments::LoginSessionSetup::Install(prefix) => {
                login_session::install(prefix.as_deref())
            }
            arguments::LoginSessionSetup::Uninstall(prefix) => {
                login_session::uninstall(prefix.as_deref())
            }
            arguments::LoginSessionSetup::ApplyChanges => login_session::apply_install_changes(),
        };
    }
    #[cfg(target_os = "linux")]
    if arguments.drm {
        let console = login_session::start_log();
        let ran = run_host(arguments, dev_token);
        if let (Err(error), Some(mut console)) = (&ran, console) {
            use std::io::Write;
            let _ = writeln!(console, "be-host: the login session ended: {error}");
        }
        return ran;
    }
    run_host(arguments, dev_token)
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn run_host(
    arguments: arguments::Arguments,
    dev_token: Option<String>,
) -> Result<(), Box<dyn Error>> {
    panic_guard::install();
    let data_dir = storage_dir().ok_or("the application-data directory is unavailable")?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|error| format!("could not make {}: {error}", data_dir.display()))?;
    let instance = match instance::claim(&data_dir)? {
        instance::Claim::First(instance) => instance,
        instance::Claim::Running(running) => return hand_over(&running, &arguments),
    };
    let mut core = HostCore::new(Some(data_dir)).map_err(|error| error.to_string())?;
    core.hear_handoffs(instance);
    core.login_session_mode = arguments.drm;
    #[cfg(target_os = "linux")]
    if arguments.drm {
        core.start_screen_lock();
    }
    let options = beui_host::native::RunOptions {
        #[cfg(target_os = "linux")]
        device_extensions: &be_lib_dmabuf::DEVICE_EXTENSIONS,
        #[cfg(target_os = "linux")]
        platforms: vec![Box::new(beui_platform_drm::Drm)],
        #[cfg(windows)]
        device_extensions: &be_host_win32_capture::DEVICE_EXTENSIONS,
        ..run_options()
    };
    if arguments.dev_workspace {
        core.open_dev_workspace(arguments.dev_server.clone(), dev_token);
    }
    if arguments.desktop {
        core.run_as_desktop();
    }
    core.warm_apps();
    beui_host_window::run_with(options, Shell::new(core))
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn hand_over(
    running: &instance::Running,
    arguments: &arguments::Arguments,
) -> Result<(), Box<dyn Error>> {
    let data = running.data_dir().display();
    if arguments.desktop || arguments.drm {
        return Err(format!(
            "another be-host is already running on {data}, and a second desktop cannot share its data; quit that one first"
        )
        .into());
    }
    running
        .hand_over(instance::Handoff {
            dev_workspace: arguments.dev_workspace,
        })
        .map_err(|error| {
            format!(
                "another be-host is already running on {data}, but {error}; quit that one first"
            )
        })?;
    eprintln!("be-host: already running on {data}; showed its window");
    Ok(())
}

/// In the host's worker (`web/be-worker.js`): runs the work the page started there under the
/// name `entry`, from the port and bytes it sent (`be_host_stack::start_in_worker`).
/// Every entry is named here.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn be_worker_run(
    entry: String,
    port: web_sys::MessagePort,
    bytes: Vec<u8>,
) -> web_sys::js_sys::Promise {
    be_wasm_threads::initialize_worker();
    let port = be_host_stack::Port::new(port);
    let work: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> = match entry.as_str() {
        be_host_stack::STACK_ENTRY => Box::pin(be_host_stack::run_stack(port, bytes)),
        host_files::ENTRY => Box::pin(host_files::keep(port, bytes)),
        _ => {
            // Dropped, so what waits on it on the page hears the worker stopped.
            drop(port);
            return web_sys::js_sys::Promise::reject(&wasm_bindgen::JsValue::from_str(&format!(
                "the host's worker has no entry named {entry}"
            )));
        }
    };
    wasm_bindgen_futures::future_to_promise(async move {
        work.await;
        Ok(wasm_bindgen::JsValue::UNDEFINED)
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_web(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
    be_wasm_threads::initialize_page();
    panic_guard::install();
    be_host_stack::discovery::load().await;
    // The page makes no file call: the host's worker reads its files and answers, while the
    // page's start-up look stays up (`#status`), then the host shows its first screen.
    // An error named `BeSavedData` tells the page (host.js) the saved data could not be read,
    // not that drawing failed.
    let (failures, report) = desk().mailbox(HostCore::host_files_failed);
    let (host_state, workspaces) = host_files::open(PathBuf::from(WEB_DATA_DIR), report)
        .await
        .map_err(|error| {
            let error = web_sys::js_sys::Error::new(&error);
            error.set_name("BeSavedData");
            wasm_bindgen::JsValue::from(error)
        })?;
    let mut core = HostCore::new(host_state, workspaces, Some(failures))
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    if let Some(status) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("status"))
    {
        status.set_attribute("hidden", "")?;
    }
    let options = run_options();
    web_push::install();
    let location = web_sys::window()
        .map(|window| window.location())
        .ok_or("no browser window is available")?;
    let page = web_sys::Url::new(&location.href()?)?;
    if page.search_params().has("dev-workspace") {
        core.open_dev_workspace(Some(location.origin()?), None);
    }
    if location.hash()?.starts_with("#invite/") {
        core.open_invite(&location.href()?);
        page.set_hash("");
        location.replace(&page.href())?;
    }
    if location.hash()?.starts_with("#contact/") {
        core.contacts.open_from_address(&location.href()?);
        page.set_hash("");
        location.replace(&page.href())?;
    }
    let mut layers: Vec<Box<dyn beui_host::native::Layer>> = Vec::new();
    if let Some(inbox) = automation()? {
        layers.push(Box::new(beui_host::native::drive::Drive::new(inbox)));
    }
    beui_host::native::run_web(&canvas_id, options, layers, Shell::new(core))
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
        canceller.cancel(beui_host::native::drive::timed_out(
            std::time::Duration::from_millis(u64::try_from(timeout_milliseconds).unwrap_or(0)),
            &inbox,
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
            Err("the web build does not answer paint; run be-host natively".into())
        }
    }
}

// unsafe(ffi)
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android: beui_host_window::AndroidApp) {
    let assets = android.clone();
    be_host_stack::discovery::use_assets(move |path| {
        use std::io::Read;
        let name = std::ffi::CString::new(path)
            .map_err(|_| "the asset path is not a C string".to_owned())?;
        let mut asset = assets.asset_manager().open(&name).ok_or("no such asset")?;
        let mut bytes = Vec::new();
        asset
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    });
    panic_guard::install();
    let storage_root = android.internal_data_path();
    if let Some(files) = &storage_root {
        android_push::install(files.clone());
    }
    let options = run_options();
    let exit_code = match HostCore::new(storage_root)
        .map_err(|error| error.to_string())
        .and_then(|core| {
            beui_host_window::run_with(options, Shell::new(core)).map_err(|error| error.to_string())
        }) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("be-host stopped: {error}");
            1
        }
    };

    std::process::exit(exit_code);
}

struct Shell {
    document: Document,
    view: HostUiStore,
    core: HostCore,
    /// Whether the last frame was hurried (`Context::hurried`), which `busy` needs: only then
    /// does the host seal what its blocks owe at once.
    hurried: bool,
}

impl Shell {
    fn new(core: HostCore) -> Self {
        let mut view = None;
        let desktop = core.desktop;
        let mut document = beui_host::native::build(|| {
            compositor::install();
            if desktop {
                Host::create();
            }
            surfaces::create_handles();
            display::create_handle();
            let store = HostUiStore::new(HostUi::default());
            view = Some(store);
            beui::reactive::view! {
                <beui::styled::Root title="be-host">
                    <ui::Displays view={store} />
                </beui::styled::Root>
            }
        });
        document.on_interacted(|| {
            be_host_runtime::start_frames(beui::native::with_document(|document| {
                document.needs_layout()
            }));
        });
        document.on_laid_out(|| {
            let relaid = be_host_runtime::settle();
            compositor::notify_after_layout();
            if relaid {
                beui::native::with_document(beui::native::Document::run_laid_out_again);
            }
        });
        Self {
            document,
            view: view.expect("the view store is created while the document is built"),
            core,
            hurried: false,
        }
    }
}

impl beui::native::Program for Shell {
    fn setup(&mut self, setup: &beui::native::Setup) {
        let waker = setup.waker.clone();
        host::install_waker(move || waker.wake());
        host::install_clock(beui::reactive::now);
        be_host_runtime::install(setup);
        if self.core.desktop {
            Host::start(setup);
        }
        input::start(setup);
        #[cfg(target_os = "linux")]
        if let Some(problems) = setup.get::<beui_platform_drm::Problems>() {
            problems.listen(notices::report);
        }
        #[cfg(target_os = "linux")]
        if self.core.login_session.is_none() && login_session::owns_a_seat(setup) {
            self.core.login_session = Some(login_session::LoginSession::start(
                desk().mailbox(HostCore::logind_heard),
            ));
            self.core.power.log_in();
            login_session::export_environment(Host::environment());
        }
        display::start(setup);
        be_host_desktop::remap::load_cached(self.core.host_state.remapper().unwrap_or_else(
            |error| {
                eprintln!(
                    "be-host: the input remapper kept for this session was not read: {error}"
                );
                None
            },
        ));
        self.core.input.boot(&self.core.host_state);
        self.core.display.boot(&self.core.host_state);
        display::boot(&self.core.host_state);
    }

    fn renderer_replaced(&mut self, setup: &beui::native::Setup) {
        be_host_runtime::replace_gpu(setup);
        Host::replace_gpu(setup);
    }

    fn update(&mut self, context: &beui::native::Context, rect: beui::Rect) {
        let started = std::time::Instant::now();
        be_host_runtime::begin_frame(
            context.next_present(),
            context.refresh_interval(),
            context.last_prepared(),
        );
        frame::begin(context);
        self.hurried = context.hurried();
        if self.hurried {
            be_host_stack::seal_owed();
        }
        if context.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, beui::Event::Focus(true)))
        }) {
            be_host_stack::retry_now();
            self.core.retry_organizations();
        }
        Host::before(context, rect, &mut self.document);
        self.core.frame();
        self.core.sync_ui_settings();
        if let Some(preferences) = self.core.preferences.take() {
            context.set_preference_layer(beui::PreferenceSource::Program, preferences);
        }
        if let Some(scale) = self.core.scale.take() {
            context.set_zoom_factor(scale);
        }
        if let Some(share) = self.core.keyboard_share.take() {
            context.set_keyboard_share(share);
        }
        if let Some(open) = self.core.inspector_requested.take()
            && open
        {
            self.document.open_inspector();
        }
        let view = self.core.view();
        let owned = self.core.desktop
            && view.screen == ui::Screen::Organization
            && self.core.shell.is_some();
        be_host_desktop::desktop_host::configure(|desktop| desktop.owned = owned);
        let store = self.view;
        compositor::notify();
        beui::native::enter(&mut self.document, || {
            // What the apps did in this frame's `poll` reaches its owners in the document.
            host::deliver();
            display::apply_shown();
            store.set(view);
        });
        #[cfg(target_arch = "wasm32")]
        be_host_runtime::place_screens(
            &be_host_runtime::shown_blits(),
            rect.size(),
            context.pixels_per_point(),
            context.pixels_per_point() / context.native_pixels_per_point(),
            beui::styled::Theme::of(self.document.preferences().theme).background,
        );
        self.document.show(context, rect);
        be_host_runtime::describe(context);
        #[cfg(target_os = "linux")]
        if let Some(lock) = &mut self.core.screen_lock {
            lock.note_drawn(self.document.locked());
        }
        Host::after(context, &mut self.document);
        let commands = ui::take_commands();
        if !commands.is_empty() {
            for command in commands {
                self.core.command(context, command);
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
        be_host_runtime::record_pacing();
    }

    fn busy(&self) -> Vec<String> {
        if self.core.error.is_some() {
            return Vec::new();
        }
        let mut reasons = self.core.busy(self.hurried);
        reasons.extend(be_host_runtime::busy());
        reasons.extend(compositor::owes_frame());
        reasons
    }

    fn out_of_step(&self) -> Option<Vec<String>> {
        Some(self.core.out_of_step())
    }

    fn connected(&self) -> Option<bool> {
        Some(be_host_stack::status().connected)
    }

    fn in_step_due(&self) -> Option<std::time::Instant> {
        be_host_stack::modules_due()
    }

    fn environment(&self) -> Vec<(String, String)> {
        Host::environment()
    }

    fn drawing(&self) -> bool {
        be_host_runtime::drawing()
    }

    fn request_painting(&mut self) {
        be_host_runtime::request_painting();
    }

    fn other_frames(&mut self) -> Vec<(String, beui::native::FrameTimes)> {
        be_host_runtime::take_steps()
    }

    fn close_requested(&mut self) -> bool {
        self.core.close_requested()
    }

    fn lifecycle(&mut self, change: beui::native::Lifecycle) {
        match change {
            beui::native::Lifecycle::Background => be_host_stack::backgrounded(),
            beui::native::Lifecycle::Foreground
            | beui::native::Lifecycle::Focused
            | beui::native::Lifecycle::NetworkChanged => be_host_stack::retry_now(),
        }
    }

    fn exiting(&mut self) {
        Host::exiting();
        be_host_runtime::exiting();
        display::stop();
        #[cfg(target_os = "linux")]
        {
            self.core.media = None;
            self.core.notifications = None;
            self.core.tray = None;
            self.core.login_session = None;
        }
        be_host_stack::flush();
        be_host_stack::stop_and_wait();
    }
}

struct HostCore {
    host_state: HostStateStore,
    /// The organizations each saved account last listed (`host_files`).
    workspaces: host_files::WorkspaceCache,
    /// Where the web's host worker tells of a change to the host's files it could not keep.
    #[cfg(target_arch = "wasm32")]
    host_files_failed: Option<host::Mailbox>,

    client_id: Uuid,
    accounts: Vec<Account>,
    signed_in: bool,
    add_account_open: bool,
    add_account_generation: u64,
    pending_account_request: Option<PendingAccountRequest>,
    #[cfg(not(target_arch = "wasm32"))]
    login_session_mode: bool,
    desktop: bool,
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
    apps_in_use: Vec<String>,
    pending_organization_request: Option<host::Mailbox>,
    organization_created: u64,
    organization_error: Option<String>,

    reauth: Option<ReauthState>,
    removing_account: Option<Account>,
    invitations: invitations::Invitations,
    agents_open: bool,
    agents: Vec<be_block_protocol::Agent>,
    agent_created: Option<accounts::AgentCredentials>,
    contacts: contacts::Contacts,
    /// What the host stack changed since the last frame.
    stack_changes: be_host_stack::ChangeWatch,
    /// The `be_host_base::push_device::revision` the push device watch last published.
    push_device_published: Option<u64>,
    /// What a pressed notification's block last waited on (the open organization, how many
    /// there are, the changes unsaved), so it is tried again only when that or the graph
    /// changes.
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    pressed_waiting_on: Option<(Option<Uuid>, usize, usize)>,
    /// The organization this device's push endpoint was last registered for.
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    push_synced: Option<Uuid>,
    members: members::Members,
    scheduled_organization_list: bool,
    server_url: String,
    account: Account,
    root_settings: RootSettings,
    set_up_profile: Option<Uuid>,
    choosing_session: bool,
    every_session_type: bool,
    shell: Option<Uuid>,
    windows_sent: Option<(u64, Vec<be_sdk_api::HostWindowId>)>,
    focus_reports: HashMap<Uuid, editors::FocusReport>,
    artifact_watches: HashMap<Uuid, Vec<Uuid>>,
    /// What instances reported that the core acts on: focus, artifact watches, and their
    /// children's commits, dismissals and closes.
    instance_events: Vec<be_host_runtime::Heard>,
    _instance_events: host::Mailbox,
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
    pending_reorders: Vec<PendingReorder>,
    pending_copies: Vec<PendingCopy>,
    about_open: bool,
    unsent_open: bool,
    discarding_unsent: Option<Uuid>,
    programs: be_host_desktop::programs::Programs,
    pending_destructive_action: Option<PendingDestructiveAction>,
    leaving: Option<Uuid>,
    /// Blocks opened at a place, which their editors are told once open (`reveal_opened`).
    reveals: reveals::Reveals,
    /// The answer to leaving `leaving`, while it is on its way, and why it was refused (offline,
    /// it needs the server), which the leave dialog shows.
    leave_answers: Option<host::Mailbox>,
    leave_refused: Option<String>,
    /// Answers to approving pairing requests, and the refusals the pairing dialog shows, by
    /// request.
    pairing_answers: HashMap<u64, host::Mailbox>,
    pairing_refused: HashMap<u64, String>,
    allow_close: bool,
    data_dir: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    _handoffs: Option<host::Mailbox>,
    error: Option<String>,
    pending_error_action: Option<ErrorAction>,
    inspector_requested: Option<bool>,
    dev_workspace: bool,
    keys: keys::KeyState,
    identity: Option<[u8; 32]>,
    #[cfg(target_os = "linux")]
    login_session: Option<login_session::LoginSession>,
    #[cfg(target_os = "linux")]
    screen_lock: Option<login_session::ScreenLock>,
    #[cfg(target_os = "linux")]
    power: login_session::Power,
    #[cfg(target_os = "linux")]
    published_power: Option<be_sdk_api::PowerAvailability>,
    #[cfg(target_os = "linux")]
    media: Option<media::Media>,
    #[cfg(target_os = "linux")]
    notifications: Option<notifications::Notifications>,
    #[cfg(target_os = "linux")]
    screenshots: Option<screenshot::Screenshots>,
    #[cfg(target_os = "linux")]
    tray: Option<tray::Tray>,
    #[cfg(target_os = "linux")]
    wifi: Option<wifi::Wifi>,
}

type Account = SavedAccount;

thread_local! {
    static DESK: host::Desk<HostCore> = host::Desk::default();
}

fn desk() -> host::Desk<HostCore> {
    DESK.with(|desk| *desk)
}

fn request<T: Send + 'static>(
    work: impl platform::Request<T>,
    answered: impl Fn(&mut HostCore, T) + 'static,
) -> host::Mailbox {
    let (mailbox, post) = desk().mailbox(answered);
    platform::spawn_request(async move { post.send(work.await) });
    mailbox
}

struct PendingAccountRequest {
    _answers: host::Mailbox,
    browser: Option<accounts::BrowserLogin>,
    _cancel: tokio::sync::oneshot::Sender<()>,
    check_now: tokio::sync::mpsc::UnboundedSender<()>,
    server: ServerLocation,
    url: String,
    authorization: Option<String>,
}

/// An invite link being opened. Its code is held across signing in or registering, so the
/// invitation is there to accept once an account is.
/// How an invite link being opened shows: whose it is and into what, and whether the account
/// signed in can join it, or why not.
fn joining_row(joining: &Joining) -> Option<ui::JoiningRow> {
    use be_block_protocol::InvitationState;
    let (organization, inviter) = match (&joining.invitation, &joining.preview) {
        (Some(invited), _) => (invited.organization_name.clone(), invited.inviter.clone()),
        (None, Some(preview)) => (preview.organization_name.clone(), preview.inviter.clone()),
        (None, None) => return None,
    };
    // An ended invitation names neither.
    let (organization, inviter) = match organization.is_empty() {
        true => ("an organization".to_owned(), "Someone".to_owned()),
        false => (organization, inviter),
    };
    let ended = match joining.preview.as_ref().map(|preview| preview.state) {
        Some(InvitationState::Accepted) => Some("This invitation was already used."),
        Some(InvitationState::Expired) => Some("This invitation expired."),
        Some(InvitationState::Revoked) => Some("This invitation was revoked."),
        _ => None,
    };
    let problem = match (ended, &joining.refused) {
        (Some(ended), _) => format!("{ended} Ask whoever invited you for a new link."),
        (None, Some(refused)) => format!(
            "{} Sign in with another account to accept it.",
            sentence(refused)
        ),
        (None, None) => String::new(),
    };
    Some(ui::JoiningRow {
        organization,
        inviter,
        can_join: joining.invitation.is_some() && problem.is_empty(),
        problem,
    })
}

/// A refusal as a sentence: capitalized, with a full stop.
fn sentence(text: &str) -> String {
    let mut characters = text.trim().chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let mut sentence: String = first.to_uppercase().chain(characters).collect();
    if !sentence.ends_with('.') {
        sentence.push('.');
    }
    sentence
}

struct Joining {
    link: InviteLink,
    /// Read before signing in: whose invitation it is, into what, and where it stands.
    preview: Option<accounts::InvitationPreview>,
    /// Read by the account signed in, which may accept it.
    invitation: Option<accounts::Invited>,
    /// Why the account signed in can't take it (already a member, say).
    refused: Option<String>,
    _previewing: host::Mailbox,
}

enum OrganizationOperation {
    Load,
    Create(String),
    LookUp(be_block_keys::InviteCode),
    Join(be_block_keys::InviteCode),
    ListAgents,
    CreateAgent(String),
    RemoveAgent(Uuid),
}

enum OrganizationResult {
    /// The organizations listed, the account's keys, and the server and account they were
    /// listed for, under which they are cached (`host_files::WorkspaceCache`).
    Loaded(Vec<Organization>, accounts::Keys, (String, Uuid)),
    Cached(Vec<Organization>),
    Created(Organization),
    Invitation(accounts::Invited),
    Joined(Uuid),
    Agents(
        Option<Vec<be_block_protocol::Agent>>,
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
    /// Puts a block back in the source a refused move took it out of: refused again, it is not
    /// reported twice.
    putting_back: bool,
    /// What the block is called as it is queued, for a refusal to name: the graph can let it go
    /// once its source does.
    name: String,
    /// Where the block sat in a source block, after which sibling (none: first), as it was
    /// queued: a block put back after a refusal goes back there when the source keeps an order.
    was_after: Option<Option<Uuid>>,
}

#[derive(Clone)]
struct PendingReorder {
    child: Uuid,
    parent: Uuid,
    after: Option<Uuid>,
    /// What the parent's type answered, once the reorder was sent.
    answer: Option<be_host_stack::ChildChanged>,
}

/// A transfer takes a block out of its source, then puts it in its destination, each through
/// the container's type, which answers whether it made the change.
#[derive(Clone)]
enum TransferStage {
    DeleteSource,
    DeletingSource(be_host_stack::ChildChanged),
    AddDestination,
    AddingDestination(be_host_stack::ChildChanged),
}

#[derive(Clone)]
struct PendingCopy {
    source: Uuid,
    container: Uuid,
    stage: CopyStage,
}

#[derive(Clone)]
enum CopyStage {
    Duplicate,
    /// The container's type was asked to put a copy, `copy_id`, in the original's place: the
    /// stack makes the copy as the type takes it (`be_host_stack::replace_with_copy`), so a
    /// refusal leaves no copy behind, and the host places it in the container once it answers.
    Replacing {
        copy_id: Uuid,
        answer: be_host_stack::ChildChanged,
    },
}

impl HostCore {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(storage_root: Option<PathBuf>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = storage_root
            .or_else(storage_dir)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?;
        std::fs::create_dir_all(&data_dir)?;
        panic_guard::report_in(&data_dir);
        be_host_runtime::cache_code_in(data_dir.join("code-cache"));
        let host_state = HostStateStore::open(data_dir.join("host.sqlite3"))?;
        let mut core = Self::with_state(host_state)?;
        core.workspaces = host_files::WorkspaceCache::open(&data_dir, &core.accounts);
        core.programs.keep_files_in(data_dir.join("files"));
        core.data_dir = data_dir;
        Ok(core)
    }

    /// With what the host's worker read of its files (`host_files::open`), and the mailbox it
    /// tells of a change it could not keep.
    #[cfg(target_arch = "wasm32")]
    fn new(
        host_state: HostStateStore,
        workspaces: host_files::WorkspaceCache,
        failures: Option<host::Mailbox>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut core = Self::with_state(host_state)?;
        core.host_files_failed = failures;
        core.workspaces = workspaces;
        core.data_dir = PathBuf::from(WEB_DATA_DIR);
        Ok(core)
    }

    /// A change to the host's files the web's host worker could not keep (out of space, a
    /// failed write), shown as a host error, as a write that fails in place is.
    #[cfg(target_arch = "wasm32")]
    fn host_files_failed(&mut self, unkept: host_files::Unkept) {
        notices::report(format!(
            "This device could not save {}: the page will undo it when it reloads ({}).",
            unkept.what, unkept.error
        ));
    }

    fn with_state(host_state: HostStateStore) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let client_id = host_state.client_id()?;
        let accounts = host_state.accounts()?;
        accounts::authorize(&accounts);
        let active = host_state.active_account()?;
        let account = active
            .and_then(|(server, id)| {
                accounts
                    .iter()
                    .find(|account| account.server.key() == server && account.id == id)
            })
            .cloned();
        let signed_in = account.is_some();
        let identity = match &account {
            Some(account) => host_state.identity(account)?,
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
        let server_url = account.server.url();
        let registry = Rc::new(EditorRegistry::new());
        let editors = compositor::OwnedEditors::install(Rc::clone(&registry), client_id);
        let mut core = Self {
            host_state,
            workspaces: host_files::WorkspaceCache::default(),
            #[cfg(target_arch = "wasm32")]
            host_files_failed: None,
            client_id,
            accounts,
            signed_in,
            add_account_open: false,
            add_account_generation: 0,
            pending_account_request: None,
            #[cfg(not(target_arch = "wasm32"))]
            login_session_mode: false,
            desktop: false,
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
            apps_in_use: Vec::new(),
            pending_organization_request: None,
            organization_created: 0,
            organization_error: None,
            reauth: None,
            removing_account: None,
            invitations: invitations::Invitations::default(),
            agents_open: false,
            agents: Vec::new(),
            agent_created: None,
            contacts: contacts::Contacts::default(),
            stack_changes: be_host_stack::ChangeWatch::new(),
            push_device_published: None,
            #[cfg(any(target_arch = "wasm32", target_os = "android"))]
            pressed_waiting_on: None,
            #[cfg(any(target_arch = "wasm32", target_os = "android"))]
            push_synced: None,
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
            instance_events: Vec::new(),
            _instance_events: {
                let (mailbox, post) = desk().mailbox(HostCore::instance_heard);
                be_host_runtime::listen(None, post);
                mailbox
            },
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
            pending_reorders: Vec::new(),
            pending_copies: Vec::new(),
            about_open: false,
            unsent_open: false,
            discarding_unsent: None,
            programs: be_host_desktop::programs::Programs::default(),
            pending_destructive_action: None,
            leaving: None,
            reveals: reveals::Reveals::default(),
            leave_answers: None,
            leave_refused: None,
            pairing_answers: HashMap::new(),
            pairing_refused: HashMap::new(),
            allow_close: false,
            data_dir: PathBuf::new(),
            #[cfg(not(target_arch = "wasm32"))]
            _handoffs: None,
            error: None,
            pending_error_action: None,
            inspector_requested: None,
            dev_workspace: false,
            keys: keys::KeyState::default(),
            identity,
            #[cfg(target_os = "linux")]
            login_session: None,
            #[cfg(target_os = "linux")]
            screen_lock: None,
            #[cfg(target_os = "linux")]
            power: login_session::Power::default(),
            #[cfg(target_os = "linux")]
            published_power: None,
            #[cfg(target_os = "linux")]
            media: None,
            #[cfg(target_os = "linux")]
            notifications: None,
            #[cfg(target_os = "linux")]
            screenshots: None,
            #[cfg(target_os = "linux")]
            tray: None,
            #[cfg(target_os = "linux")]
            wifi: None,
        };
        core.resume_organization();
        Ok(core)
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    fn run_as_desktop(&mut self) {
        self.desktop = true;
        be_host_runtime::run_as_desktop();
        self.root_settings = RootSettings::new(be_block::DESKTOP_EDITOR);
        be_host_base::watches::publish::<be_sdk_api::DesktopBarWatch>(&true);
        #[cfg(windows)]
        be_host_base::watches::publish::<be_sdk_api::SuperKeptWatch>(&true);
        #[cfg(target_os = "linux")]
        {
            memory::start();
            self.start_screen_lock();
            if self.notifications.is_none() {
                self.notifications = Some(notifications::Notifications::start());
            }
            if self.tray.is_none() {
                self.tray = Some(tray::Tray::start());
            }
            if self.screenshots.is_none() {
                self.screenshots = Some(screenshot::Screenshots::start(
                    be_host_desktop::desktop_host::screenshot,
                ));
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn start_screen_lock(&mut self) {
        if self.screen_lock.is_none() {
            self.screen_lock = Some(login_session::ScreenLock::start());
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
        self.run_media();
        self.run_wifi();
        display::run_epaper();
        display::run_screen_input();
        input::run_layout_requests();
    }

    #[cfg(target_os = "linux")]
    fn request_power(&mut self, action: be_sdk_api::PowerAction, trigger: login_session::Trigger) {
        match action {
            be_sdk_api::PowerAction::Lock => {
                if let Some(lock) = &mut self.screen_lock {
                    lock.lock(trigger);
                }
            }
            action => {
                self.power.request(
                    action,
                    &mut login_session::Live::of(self.login_session.as_ref()),
                );
                host::request_repaint();
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn run_power(&mut self, locked: bool) {
        let request = be_host_base::watches::take_calls(be_sdk_api::requested::power).pop();
        if let Some(action) = request
            && self
                .published_power
                .is_some_and(|power| power.allows(action))
        {
            self.request_power(action, login_session::Trigger::Menu);
        }
        for answer in be_host_base::watches::take_calls(be_sdk_api::requested::ending_answer) {
            self.power.answer(answer);
        }
        let mut availability = self.power.availability();
        availability.lock = !locked;
        if self.published_power != Some(availability) {
            self.published_power = Some(availability);
            be_host_base::watches::publish::<be_sdk_api::PowerWatch>(&availability);
        }
        self.power
            .frame(&mut login_session::Live::of(self.login_session.as_ref()));
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    fn hear_handoffs(&mut self, instance: instance::Instance) {
        let (mailbox, post) = desk().mailbox(Self::handed_over);
        self._handoffs = Some(mailbox);
        instance.serve(move |handoff| post.send(handoff));
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    fn handed_over(&mut self, handoff: instance::Handoff) {
        if handoff.dev_workspace {
            self.open_dev_workspace(None, None);
        }
        host::raise_window();
        host::request_repaint();
    }

    #[cfg(target_os = "linux")]
    fn logind_heard(&mut self, event: login_session::LogindEvent) {
        if let (Some(login_session), Some(lock)) = (&mut self.login_session, &mut self.screen_lock)
        {
            login_session.heard(event, lock, &mut self.power);
        }
        host::request_repaint();
    }

    #[cfg(target_os = "linux")]
    fn run_screen_lock(&mut self) {
        let Some(lock) = &mut self.screen_lock else {
            return;
        };
        lock.frame();
        if Host::take_lock_due() {
            lock.lock(login_session::Trigger::Idle);
        }
        if let Some(login_session) = &mut self.login_session {
            login_session.frame(lock);
        }
        let locked = lock.locked();
        Host::set_locked(locked);
        be_host_desktop::remap::set_locked(locked);
        be_host_base::watches::publish::<be_sdk_api::ScreenLockedWatch>(&locked);
        self.run_power(locked);
    }

    #[cfg(target_os = "linux")]
    fn lock_view(&self) -> ui::LockView {
        let Some(lock) = &self.screen_lock else {
            return ui::LockView::default();
        };
        let state = lock.state();
        let offered = self.power.availability();
        ui::LockView {
            available: true,
            locked: state.locked,
            busy: state.busy,
            error: state.error,
            user: lock.user().to_owned(),
            power: [
                be_sdk_api::PowerAction::Suspend,
                be_sdk_api::PowerAction::Restart,
                be_sdk_api::PowerAction::PowerOff,
            ]
            .into_iter()
            .filter(|action| offered.allows(*action))
            .collect(),
        }
    }

    #[cfg(target_os = "linux")]
    fn run_media(&mut self) {
        let requests = be_host_base::watches::take_calls(be_sdk_api::requested::media);
        if self.media.is_none()
            && (!requests.is_empty() || be_host_base::watches::watched::<be_sdk_api::MediaWatch>())
        {
            self.media = Some(media::Media::start());
        }
        let Some(media) = &mut self.media else {
            return;
        };
        for request in requests {
            media.request(request);
        }
    }

    #[cfg(target_os = "linux")]
    fn run_wifi(&mut self) {
        let requests = be_host_base::watches::take_calls(be_sdk_api::requested::wifi);
        if self.wifi.is_none()
            && (!requests.is_empty() || be_host_base::watches::watched::<be_sdk_api::WifiWatch>())
        {
            self.wifi = Some(wifi::Wifi::start());
        }
        let Some(wifi) = &mut self.wifi else {
            return;
        };
        for request in requests {
            wifi.request(request);
        }
    }

    /// Why a driver should wait: the host's own work and the block store's, nothing once an
    /// error screen waits for a person. `hurried` as for `be_host_stack::busy`.
    fn busy(&self, hurried: bool) -> Vec<String> {
        let mut reasons = self.own_busy();
        if self.error.is_none() {
            reasons.extend(be_host_stack::busy(hurried));
        }
        reasons
    }

    /// Why the open workspace is not in step with its server (drive's `synced`).
    fn out_of_step(&self) -> Vec<String> {
        if let Some(error) = &self.error {
            return vec![format!("the host shows an error: {error:?}")];
        }
        if self.keys.needs_unlock(self.identity) {
            return vec!["this device waits to be unlocked with the account's key".to_owned()];
        }
        if self.organization.is_none() {
            return vec!["no workspace is open".to_owned()];
        }
        be_host_stack::unsynced()
    }

    /// The host's own reasons to be busy, apart from the block store's, which is shared by
    /// everything in the process and works on its own schedule.
    fn own_busy(&self) -> Vec<String> {
        if self.error.is_some() {
            return Vec::new();
        }
        let signing_in = self
            .pending_account_request
            .as_ref()
            .is_some_and(|pending| pending.browser.is_none());
        [
            (signing_in, "an account request is pending"),
            (
                self.pending_organization_request.is_some(),
                "an organization request is pending",
            ),
            (!self.pending_transfers.is_empty(), "a transfer is pending"),
            (!self.pending_reorders.is_empty(), "a reorder is pending"),
            (!self.pending_copies.is_empty(), "a copy is pending"),
            (
                // A device of the account without its key waits on the unlock screen for a
                // person (or a test) to pair it, which is not work left to do.
                self.dev_workspace
                    && self.organization.is_none()
                    && !self.keys.needs_unlock(self.identity),
                "the dev workspace has yet to open",
            ),
        ]
        .into_iter()
        .filter_map(|(busy, reason)| busy.then_some(reason))
        .map(str::to_owned)
        .collect()
    }

    /// Signs in to the dev workspace's account: a local account, made on this device if there is
    /// none, or at `remote_url` with `token` when given (a test's server issues one per device of
    /// an account, crates/be-host-tests), and otherwise through the browser.
    #[cfg(not(target_os = "android"))]
    fn open_dev_workspace(&mut self, remote_url: Option<String>, token: Option<String>) {
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
        self.add_account_open = remote_url.is_some() && token.is_none();
        self.begin_account_request(
            AccountForm {
                remote: remote_url.is_some(),
                remote_url: remote_url.unwrap_or_default(),
                authorization: String::new(),
                display_name: "Dev".to_owned(),
            },
            token,
        );
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
        // The person's own link reads nothing: opening it would mark it opened for them.
        if self.signed_in
            && be_host_stack::is_own_invitation(&self.server_url, self.account.id, &link.code)
        {
            self.organization_error = Some(
                "This is your own invite link: send it to the person you want to invite."
                    .to_owned(),
            );
            return;
        }
        // Read before signing in at the server the link names, so whoever has no account
        // here yet sees who invited them and to what.
        let server = link
            .server
            .clone()
            .unwrap_or_else(|| accounts::link_base(&self.server_url));
        let code = link.code.clone();
        self.joining = Some(Joining {
            link,
            preview: None,
            invitation: None,
            refused: None,
            _previewing: request(
                accounts::preview_invitation(server, code),
                HostCore::invitation_previewed,
            ),
        });
    }

    fn invitation_previewed(
        &mut self,
        previewed: Result<accounts::InvitationPreview, AccountError>,
    ) {
        match previewed {
            Ok(preview) => {
                if let Some(joining) = &mut self.joining {
                    joining.preview = Some(preview);
                }
                if self.signed_in && self.organizations_loaded && !self.keys.needs_recovery() {
                    self.look_up_invite();
                }
            }
            Err(error) => {
                self.joining = None;
                self.organization_error = Some(error.message);
            }
        }
    }

    fn look_up_invite(&mut self) {
        let Some(joining) = &self.joining else {
            return;
        };
        // Looked up once a preview says it is open, and once per account signed in.
        let open = joining.preview.as_ref().is_some_and(|preview| {
            matches!(
                preview.state,
                be_block_protocol::InvitationState::Waiting
                    | be_block_protocol::InvitationState::Opened
            )
        });
        if joining.invitation.is_some() || joining.refused.is_some() || !open {
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

    /// Signs in: as a new local account made on this device (`create_local_account`), or at the
    /// form's server with `token` when given, and through the browser otherwise.
    fn begin_account_request(&mut self, form: AccountForm, token: Option<String>) {
        if !form.remote {
            self.create_local_account(form.display_name);
            return;
        }
        let url = form.remote_url.trim().trim_end_matches('/').to_owned();
        if url.is_empty() {
            self.account_error = Some("a server address is required".to_owned());
            return;
        }
        let server = ServerLocation::Remote(url.clone());
        let authorization =
            Some(form.authorization.trim().to_owned()).filter(|header| !header.is_empty());
        be_block_client::authorize(&url, authorization.clone());
        let requested = url.clone();
        let default = platform::default_account_server();
        let auth_url = default
            .filter(|server| server.api_url == url)
            .map(|server| server.auth_url)
            .unwrap_or_else(|| format!("{}/auth", accounts::link_base(&url)));
        let (answers, post) = desk().mailbox(HostCore::account_answered);
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let (check_now, checks) = tokio::sync::mpsc::unbounded_channel();
        platform::spawn_request(async move {
            let notify = post.clone();
            let result = if let Some(token) = token {
                accounts::token_login(requested, token)
                    .await
                    .map_err(|error| error.to_string())
            } else {
                accounts::browser_login(
                    requested,
                    auth_url,
                    move |browser| notify.send(accounts::LoginEvent::Browser(browser)),
                    checks,
                    Box::pin(async {
                        let _ = cancelled.await;
                    }),
                )
                .await
            };
            post.send(accounts::LoginEvent::Finished(result));
        });
        self.account_error = None;
        self.pending_account_request = Some(PendingAccountRequest {
            _answers: answers,
            browser: None,
            _cancel: cancel,
            check_now,
            server,
            url,
            authorization,
        });
    }

    /// A local account, which has no server (guides/hosting.md, Local accounts and the default server): its id, its identity and
    /// its one organization are made on this device, the organization under the account's own
    /// id, and it signs in at once. Its key is kept in plaintext beside its data, as a device
    /// that holds everything it has has nothing a phrase would protect.
    fn create_local_account(&mut self, display_name: String) {
        let id = Uuid::now_v7();
        let saved = SavedAccount {
            server: ServerLocation::Local,
            id,
            email: String::new(),
            name: display_name,
            token: String::new(),
            last_organization_id: None,
            authorization: None,
        };
        let identity = be_block_keys::RecoveryPhrase::generate()
            .secret()
            .to_bytes();
        let kept = self
            .host_state
            .save_account(&saved)
            .and_then(|()| self.host_state.set_identity(&saved, identity));
        if let Err(error) = kept {
            self.account_error = Some(error.to_string());
            return;
        }
        self.accounts = self.host_state.accounts().unwrap_or_else(|error| {
            self.account_error = Some(error.to_string());
            Vec::new()
        });
        self.account_error = None;
        self.server_url = String::new();
        self.add_account_open = false;
        self.switch_account(saved);
    }

    fn account_answered(&mut self, event: accounts::LoginEvent) {
        let result = match event {
            accounts::LoginEvent::Browser(browser) => {
                #[cfg(not(target_arch = "wasm32"))]
                if !self.login_session_mode {
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
                if let Err(error) = self.host_state.save_account(&saved) {
                    self.account_error = Some(error.to_string());
                    return;
                }
                self.accounts = self.host_state.accounts().unwrap_or_else(|error| {
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
            if let Err(error) = self.host_state.set_last_organization(&account, None) {
                self.account_error = Some(error.to_string());
            }
        }
        self.switch_account(account);
    }

    fn log_out_account(&mut self, account: &Account) {
        if account.server == ServerLocation::Local {
            self.removing_account = Some(account.clone());
            return;
        }
        self.forget_account(account);
    }

    fn forget_account(&mut self, account: &Account) {
        let url = account.server.url();
        let token = account.token.clone();
        // A local account has no server to sign out of.
        if account.server != ServerLocation::Local {
            #[cfg(target_arch = "wasm32")]
            web_push::sign_out(url, token, self.accounts.len() <= 1);
            #[cfg(target_os = "android")]
            android_push::sign_out(&self.host_state, url, token, self.accounts.len() <= 1);
            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
            platform::spawn_request(async move {
                let _ = accounts::logout(url, token, None).await;
            });
        }

        if let Err(error) = self.host_state.remove_account(account) {
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
        let account = self.account.id;
        let cached = self.workspaces.workspaces(&url, account);
        // A local account's one organization, and its key, are on this device: nothing to ask.
        let local = (self.account.server == ServerLocation::Local).then(|| {
            let keys = accounts::Keys {
                account,
                recovery: self.identity.map(|identity| {
                    be_block_keys::RecoverySecret::from_bytes(identity)
                        .identity()
                        .public
                }),
                sealed: Vec::new(),
                own: Vec::new(),
                logs: Vec::new(),
            };
            let organization = Organization {
                id: account,
                name: "Personal".to_owned(),
                kind: be_block_protocol::OrganizationKind::Personal,
                role: OrganizationRole::Editor,
                escrows: Vec::new(),
            };
            (organization, keys)
        });
        let work = async move {
            if let Some((organization, keys)) = local {
                return match operation {
                    OrganizationOperation::Load => Ok(OrganizationResult::Loaded(
                        vec![organization],
                        keys,
                        (url, account),
                    )),
                    _ => Err(AccountError {
                        message: "A local account has no server to do that with.".to_owned(),
                        invalid_token: false,
                    }),
                };
            }
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
                            let keys = accounts::keys(url.clone(), token.clone()).await?;
                            // Shown with or without an organization open, so a new account
                            // sees them too; while one is open, its stack follows changes.
                            if let Ok(listed) = accounts::account_notices(url.clone(), token).await
                            {
                                notices::account_notices(listed);
                            }
                            Ok(OrganizationResult::Loaded(
                                organizations,
                                keys,
                                (url, account),
                            ))
                        }
                        Err(error) if !error.invalid_token => match cached.is_empty() {
                            true => Err(error),
                            false => Ok(OrganizationResult::Cached(cached)),
                        },
                        Err(error) => Err(error),
                    }
                }
                OrganizationOperation::Create(name) => {
                    accounts::create_organization(url, token, me()?, name)
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
                OrganizationOperation::ListAgents => be_host_stack::agents()
                    .await
                    .map(|agents| OrganizationResult::Agents(Some(agents), None))
                    .map_err(|message| AccountError {
                        message,
                        invalid_token: false,
                    }),
                OrganizationOperation::CreateAgent(name) => {
                    let created = be_host_stack::create_agent(name).await.map_err(|message| {
                        AccountError {
                            message,
                            invalid_token: false,
                        }
                    })?;
                    let agents = be_host_stack::agents().await.ok();
                    Ok(OrganizationResult::Agents(agents, Some(created)))
                }
                OrganizationOperation::RemoveAgent(account) => be_host_stack::remove_agent(account)
                    .await
                    .map(|agents| OrganizationResult::Agents(Some(agents), None))
                    .map_err(|message| AccountError {
                        message,
                        invalid_token: false,
                    }),
            }
        };
        self.organization_error = None;
        self.pending_organization_request = Some(request(work, HostCore::organization_answered));
    }

    fn organization_answered(
        &mut self,
        result: Result<OrganizationResult, OrganizationRequestError>,
    ) {
        if !self.signed_in {
            return;
        }
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
            Ok(OrganizationResult::Loaded(organizations, keys, (url, account))) => {
                self.workspaces.set(&url, account, organizations.clone());
                self.organizations = organizations;
                self.organizations_loaded = true;
                self.organizations_load_failed = false;
                self.organizations_cached = false;
                self.keys.loaded(keys);
                if self.keys.needs_recovery() {
                    // A local account's key sits in plaintext beside its database, so
                    // there is nothing to lose and no phrase to make the user save.
                    if self.dev_workspace || self.account.server == ServerLocation::Local {
                        self.keys.skip_confirmation(
                            self.server_url.clone(),
                            self.account.token.clone(),
                            self.identity,
                            Self::recovery_saved(),
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
                // The organization already open is not opened again: that restarts its stack
                // under the first start's requests that are still on their way.
                if let Some(last_organization_id) = self.account.last_organization_id
                    && self.organization.as_ref().map(|open| open.id) != Some(last_organization_id)
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

            Ok(OrganizationResult::Agents(agents, created)) => {
                if let Some(agents) = agents {
                    self.agents = agents;
                }
                if created.is_some() {
                    self.agent_created = created;
                }
            }
            Err(error) if error.invalid_token => self.begin_reauth(),
            // A preview read is kept: the account signed in can't take the invitation, and is
            // told whose it is and offered another account.
            Err(error)
                if self.joining.as_ref().is_some_and(|joining| {
                    joining.preview.is_some() && joining.invitation.is_none()
                }) =>
            {
                if let Some(joining) = &mut self.joining {
                    joining.refused = Some(error.message);
                }
            }
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
                "This local account is no longer available. Create a new local account.".into(),
            );
            return;
        };
        self.add_account_open = true;
        self.begin_account_request(
            AccountForm {
                remote: true,
                remote_url: url,
                authorization: account.authorization.unwrap_or_default(),
                display_name: String::new(),
            },
            None,
        );
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
                    platform::spawn_request(async move {
                        let _ = accounts::logout(url, token, None).await;
                    });
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
                if let Err(error) = self.host_state.save_account(&updated) {
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

    /// Leaving answered: done, or refused with why, which the leave dialog shows.
    fn left(&mut self, left: Result<(), String>) {
        self.leave_answers = None;
        match left {
            Ok(()) => {
                self.leaving = None;
                self.leave_refused = None;
            }
            Err(refused) => self.leave_refused = Some(refused),
        }
    }

    /// Approving a pairing request answered: a refusal stays on the request's dialog.
    fn pairing_approved(&mut self, from: u64, approved: Result<(), String>) {
        self.pairing_answers.remove(&from);
        match approved {
            Ok(()) => self.pairing_refused.remove(&from),
            Err(refused) => self.pairing_refused.insert(from, refused),
        };
    }

    fn close_reauth(&mut self) {
        self.reauth = None;
        self.signed_in = false;
        if let Err(error) = self.host_state.clear_active_account() {
            self.account_error = Some(error.to_string());
        }
    }

    fn resume_organization(&mut self) {
        if !self.signed_in || self.identity.is_none() || self.organization.is_some() {
            return;
        }
        let last = self
            .host_state
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
                    .host_state
                    .set_last_organization_details(&self.account, &listed);
                self.organization = Some(listed);
                self.publish_administrator();
            }
            None => {
                let mut account = self.account.clone();
                account.last_organization_id = None;
                let _ = self.host_state.set_last_organization(&account, None);
                self.switch_account(account);
            }
        }
        true
    }

    fn open_organization(&mut self, organization: Organization) {
        self.resumed = false;
        self.focus_reports.clear();
        self.artifact_watches.clear();
        be_host_stack::stop();
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
            .host_state
            .set_last_organization(&self.account, Some(organization.id))
            .and_then(|()| {
                self.host_state
                    .set_last_organization_details(&self.account, &organization)
            })
        {
            self.organization_error = Some(error.to_string());
        }
    }

    fn recovery_saved() -> keys::Answer<()> {
        desk().mailbox(|core: &mut HostCore, saved| {
            let event = core.keys.saved(saved);
            core.key_event(event);
        })
    }

    fn paired() -> keys::Answer<[u8; 32]> {
        desk().mailbox(|core: &mut HostCore, paired| {
            let event = core.keys.paired(paired);
            core.key_event(event);
        })
    }

    fn key_event(&mut self, event: Option<keys::KeyEvent>) {
        if !self.signed_in {
            return;
        }
        let secret = match event {
            Some(keys::KeyEvent::RecoverySaved(secret) | keys::KeyEvent::Unlocked(secret)) => {
                secret
            }
            None => return,
        };
        if let Err(error) = self.host_state.set_identity(&self.account, secret) {
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
        let connected = be_host_stack::status().connected;
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
    }

    fn switch_account(&mut self, account: Account) {
        let server_url = account.server.url();
        self.focus_reports.clear();
        self.artifact_watches.clear();
        be_host_stack::stop();
        self.block_types.clear();
        self.registry = Rc::new(EditorRegistry::new());
        self.editors.reset(Rc::clone(&self.registry));
        self.watched_artifacts.clear();
        self.dynamic_artifact_sessions.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.pending_transfers.clear();
        self.pending_reorders.clear();
        self.about_open = false;
        self.unsent_open = false;
        self.discarding_unsent = None;
        self.pending_destructive_action = None;
        self.allow_close = false;
        self.organization = None;
        self.organizations.clear();
        // A held invite link is looked up again for the account signed in now.
        if let Some(joining) = &mut self.joining {
            joining.invitation = None;
            joining.refused = None;
        }
        self.resumed = false;
        self.organizations_loaded = false;
        self.organizations_load_failed = false;
        self.organizations_cached = false;
        self.pending_organization_request = None;
        self.organization_error = None;
        self.reauth = None;
        self.invitations = invitations::Invitations::default();
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
        self.identity = self.host_state.identity(&account).unwrap_or_else(|error| {
            self.organization_error = Some(error.to_string());
            None
        });
        self.account = account;
        self.server_url = server_url;
        self.signed_in = true;
        if let Err(error) = self.host_state.set_active_account(&self.account) {
            self.account_error = Some(error.to_string());
        }
        self.resume_organization();
    }

    fn close_requested(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        if !self.allow_close
            && !self
                .power
                .close(&mut login_session::Live::of(self.login_session.as_ref()))
        {
            host::request_repaint();
            return false;
        }
        if be_host_stack::flush() == 0 || self.allow_close {
            return true;
        }
        #[cfg(target_os = "linux")]
        self.power.stay();
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
            be_host_stack::node(id)
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
        if refused_offline(parent) {
            return;
        }
        if child == parent
            || self
                .pending_transfers
                .iter()
                .any(|pending| pending.child == child && pending.destination == Some(parent))
        {
            return;
        }
        self.block_types.insert(child, block_type);
        if is_unfiled(parent) {
            if !linked {
                be_host_stack::set_parent(child, BlockParent::Block(parent));
            }
            return;
        }
        if !self.ensure_editor(parent) {
            return;
        }
        if !linked {
            be_host_stack::placing(child, true);
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: None,
            destination: Some(parent),
            parent_after: (!linked).then_some(BlockParent::Block(parent)),
            stage: TransferStage::AddDestination,
            putting_back: false,
            name: name_of(child, "that block"),
            was_after: None,
        });
    }

    fn set_block_parent(&mut self, id: Uuid, parent: BlockParent) {
        be_host_stack::set_parent(id, parent);
    }

    fn queue_move(
        &mut self,
        child: Uuid,
        block_type: Uuid,
        source: SidebarDragSource,
        destination: Uuid,
        is_reference: bool,
    ) {
        if refused_offline(destination)
            || matches!(source, SidebarDragSource::Block(source) if refused_offline(source))
        {
            return;
        }
        if self
            .pending_transfers
            .iter()
            .any(|pending| pending.child == child && pending.destination == Some(destination))
        {
            return;
        }
        self.block_types.insert(child, block_type);
        let source_ready = match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned | SidebarDragSource::Unfiled => {
                true
            }
            SidebarDragSource::Block(source) => self.ensure_editor(source),
        };
        if !source_ready || !(is_unfiled(destination) || self.ensure_editor(destination)) {
            return;
        }
        if source == SidebarDragSource::Orphaned && !is_reference && !is_unfiled(destination) {
            be_host_stack::placing(child, true);
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: Some(source),
            destination: Some(destination),
            parent_after: (!is_reference).then_some(BlockParent::Block(destination)),
            stage: TransferStage::DeleteSource,
            putting_back: false,
            name: name_of(child, "that block"),
            was_after: match source {
                SidebarDragSource::Block(source) => sibling_before(source, child),
                _ => None,
            },
        });
    }

    fn queue_reorder(&mut self, child: Uuid, parent: Uuid, after: Option<Uuid>) {
        if child == parent || after == Some(child) || refused_offline(parent) {
            return;
        }
        let queued = self.pending_reorders.iter().any(|reorder| {
            (reorder.child, reorder.parent, reorder.after) == (child, parent, after)
        });
        if queued || !self.ensure_editor(parent) {
            return;
        }
        self.pending_reorders.push(PendingReorder {
            child,
            parent,
            after,
            answer: None,
        });
    }

    fn process_pending_reorders(&mut self) {
        let pending = std::mem::take(&mut self.pending_reorders);
        for mut reorder in pending {
            if let Some(answer) = &reorder.answer {
                match answer.answer() {
                    None => self.pending_reorders.push(reorder),
                    Some(false) => refuse_reorder(reorder.parent, reorder.child),
                    Some(true) => {}
                }
                continue;
            }
            let moving = self
                .pending_transfers
                .iter()
                .any(|transfer| transfer.child == reorder.child);
            if !moving {
                reorder.answer = self
                    .with_editor(reorder.parent, |editor| {
                        editor.reorder_child(reorder.child, reorder.after)
                    })
                    .flatten();
            }
            self.pending_reorders.push(reorder);
        }
    }

    fn restore(&mut self, id: Uuid) {
        let Some(root) = be_host_stack::with_graph(|graph| graph.deleted_root(id)).flatten() else {
            return;
        };
        let Some(node) = be_host_stack::node(root) else {
            return;
        };
        let usable = |parent: Uuid| {
            be_host_stack::node(parent).is_some_and(|parent| parent.access.can_edit())
                && be_host_stack::with_graph(|graph| graph.deleted_root(parent).is_none())
                    .unwrap_or(false)
        };
        let addable = |parent: Uuid| {
            be_host_stack::node(parent)
                .and_then(|parent| be_host_stack::discovery::block_type(parent.content_type))
                .is_some_and(|block_type| block_type.children.add)
        };
        let restoring = Restoring {
            parent: node.parent,
            deleted_from: node.metadata.get(&be_block_content::DELETED_FROM),
            unfiled: self.root_settings.unfiled(),
            root_addable: addable(root),
        };
        let destination = match restore_to(&restoring, usable, addable) {
            RestoreTo::Nowhere => return,
            RestoreTo::Placed(parent) => {
                self.queue_placement(root, node.content_type, parent, false);
                return;
            }
            RestoreTo::Root => {
                be_host_stack::set_parent(root, BlockParent::Root);
                return;
            }
            RestoreTo::Unfiled(unfiled) => {
                be_host_stack::set_parent(root, BlockParent::Block(unfiled));
                return;
            }
            RestoreTo::Into(destination) => destination,
        };
        let listed = be_host_stack::node(destination)
            .is_some_and(|parent| parent.references.contains(&root));
        match listed {
            true => be_host_stack::set_parent(root, BlockParent::Block(destination)),
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
        if let SidebarDragSource::Block(source) = source
            && refused_offline(source)
        {
            return;
        }
        if self.pending_transfers.iter().any(|pending| {
            pending.child == child
                && pending.source == Some(source)
                && pending.destination.is_none()
        }) {
            return;
        }
        self.block_types.insert(child, block_type);
        let ready = match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned | SidebarDragSource::Unfiled => {
                true
            }
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
            putting_back: false,
            name: name_of(child, "that block"),
            was_after: None,
        });
    }

    fn process_pending_transfers(&mut self) {
        let pending = std::mem::take(&mut self.pending_transfers);
        for mut transfer in pending {
            // The transfer lives only in memory: leaving the source before the destination can
            // take the block would lose it from both on a restart.
            if matches!(transfer.stage, TransferStage::DeleteSource)
                && transfer.destination.is_some_and(|destination| {
                    !is_unfiled(destination) && be_host_stack::out_of_reach(destination)
                })
            {
                self.pending_transfers.push(transfer);
                continue;
            }
            if matches!(transfer.stage, TransferStage::DeleteSource) {
                transfer.stage = match transfer.source {
                    None | Some(SidebarDragSource::Orphaned) => TransferStage::AddDestination,
                    Some(SidebarDragSource::Unfiled) if transfer.destination.is_some() => {
                        TransferStage::AddDestination
                    }
                    Some(SidebarDragSource::Root | SidebarDragSource::Unfiled) => {
                        be_host_stack::set_parent(transfer.child, BlockParent::Detached);
                        TransferStage::AddDestination
                    }
                    Some(SidebarDragSource::Block(source)) => {
                        let deleting = self
                            .with_editor(source, |editor| editor.delete_child(transfer.child))
                            .flatten();
                        let Some(answer) = deleting else {
                            self.pending_transfers.push(transfer);
                            continue;
                        };
                        TransferStage::DeletingSource(answer)
                    }
                };
            }
            if let TransferStage::DeletingSource(answer) = &transfer.stage {
                match answer.answer() {
                    None => {
                        self.pending_transfers.push(transfer);
                        continue;
                    }
                    Some(false) => {
                        self.refused_transfer(transfer, false);
                        continue;
                    }
                    Some(true) => transfer.stage = TransferStage::AddDestination,
                }
            }
            if matches!(transfer.stage, TransferStage::AddDestination)
                && let Some(destination) = transfer.destination
                && !is_unfiled(destination)
            {
                let adding = self
                    .with_editor(destination, |editor| editor.add_child(transfer.child))
                    .flatten();
                let Some(answer) = adding else {
                    self.pending_transfers.push(transfer);
                    continue;
                };
                transfer.stage = TransferStage::AddingDestination(answer);
            }
            if let TransferStage::AddingDestination(answer) = &transfer.stage {
                match answer.answer() {
                    None => {
                        self.pending_transfers.push(transfer);
                        continue;
                    }
                    Some(false) => {
                        self.refused_transfer(transfer, true);
                        continue;
                    }
                    Some(true) => {}
                }
            }

            // A block put back goes where it sat, when its source keeps an order and still
            // holds the sibling it followed: adding put it last.
            if transfer.putting_back
                && let (Some(after), Some(source)) = (transfer.was_after, transfer.destination)
                && self.keeps_order(source)
                && after.is_none_or(|sibling| {
                    be_host_stack::node(source)
                        .is_some_and(|node| node.contents().contains(&sibling))
                })
            {
                self.queue_reorder(transfer.child, source, after);
            }
            if let Some(parent) = transfer.parent_after {
                self.set_block_parent(transfer.child, parent);
            }
            if !matches!(transfer.parent_after, Some(BlockParent::Block(_))) {
                be_host_stack::placing(transfer.child, false);
            }
        }
    }

    /// Whether `block`'s type keeps an order of its children (its manifest's
    /// `children.reorder`).
    fn keeps_order(&self, block: Uuid) -> bool {
        self.block_type_of(block)
            .and_then(be_host_stack::discovery::block_type)
            .is_some_and(|block_type| block_type.children.reorder)
    }

    /// A container's type refused a transfer's change: out of its source (`into` false) or into
    /// its destination. The block goes back where the file tree showed it, its source taking it
    /// again when it already let it go, and the person is told why.
    fn refused_transfer(&mut self, transfer: PendingTransfer, into: bool) {
        be_host_stack::placing(transfer.child, false);
        if into {
            match transfer.source {
                Some(SidebarDragSource::Block(source)) => {
                    self.pending_transfers.push(PendingTransfer {
                        child: transfer.child,
                        source: None,
                        destination: Some(source),
                        parent_after: None,
                        stage: TransferStage::AddDestination,
                        putting_back: true,
                        name: transfer.name.clone(),
                        was_after: transfer.was_after,
                    });
                }
                Some(SidebarDragSource::Root) => {
                    be_host_stack::set_parent(transfer.child, BlockParent::Root);
                }
                Some(SidebarDragSource::Unfiled | SidebarDragSource::Orphaned) | None => {}
            }
        }
        if transfer.putting_back {
            if let Some(source) = transfer.destination {
                refuse_put_back(source, &transfer.name);
            }
            return;
        }
        let container = match (into, transfer.destination, transfer.source) {
            (true, Some(destination), _) => destination,
            (false, _, Some(SidebarDragSource::Block(source))) => source,
            _ => return,
        };
        refuse_transfer(container, &transfer.name, into);
    }

    fn queue_copy(&mut self, source: Uuid, container: Uuid) {
        if refused_offline(container) {
            return;
        }
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
                if !self.ensure_editor(copy.source) || !self.ensure_editor(copy.container) {
                    self.pending_copies.push(copy);
                    continue;
                }
                match self
                    .with_editor(copy.container, |editor| editor.takes_replace())
                    .flatten()
                {
                    None => {
                        self.pending_copies.push(copy);
                        continue;
                    }
                    Some(false) => {
                        refuse_copy(copy.container);
                        continue;
                    }
                    Some(true) => {}
                }
                // The stack makes the copy as the container takes it, before it answers.
                let replacing = self
                    .with_editor(copy.container, |editor| editor.copy_in_place(copy.source))
                    .flatten();
                let Some((copy_id, answer)) = replacing else {
                    self.pending_copies.push(copy);
                    continue;
                };
                copy.stage = CopyStage::Replacing { copy_id, answer };
            }
            let CopyStage::Replacing { copy_id, answer } = &copy.stage else {
                unreachable!("copy stage was just set to Replacing")
            };
            let copy_id = *copy_id;
            match answer.answer() {
                None => {
                    self.pending_copies.push(copy);
                    continue;
                }
                // Refused before any copy was made: nothing is left behind.
                Some(false) => {
                    refuse_copy(copy.container);
                    continue;
                }
                Some(true) => {}
            }
            // Made: the copy exists, detached, and goes in the container now. A host stopped
            // before this leaves it detached, and the container still names it.
            let made = be_host_stack::node(copy_id).map(|node| node.content_type);
            let Some(block_type) = made.or_else(|| self.block_type_of(copy.source)) else {
                self.pending_copies.push(copy);
                continue;
            };
            self.set_block_parent(copy_id, BlockParent::Block(copy.container));
            self.show_block(
                Some(copy.container),
                BlockOpen {
                    id: copy_id,
                    block_type,
                    via: Some(copy.container),
                    editor: None,
                    at: None,
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
        self.root_settings.ensure_unfiled();
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
        let view_type = be_host_stack::node(id)?.content_type;
        self.block_types.insert(id, view_type);
        let open_as = self
            .editors
            .with(|open| open.get(&id).and_then(AppEditor::editor_id));
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
        be_host_runtime::set_shell(None);
    }

    fn watch_shell(&mut self) {
        let stopped = be_host_runtime::take_stopped();
        let Some(shell) = self.shell else {
            return;
        };
        let app = self
            .with_editor(shell, |editor| {
                editor.app().map(|app| app.identity.id.clone())
            })
            .flatten();
        let Some(app) = app.filter(|app| stopped.contains(app)) else {
            return;
        };
        self.stop_shell();
        be_host_runtime::kill(&app);
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

    /// Opens the blocks notifications named that were pressed, switching to their
    /// organization first. One that cannot open yet (the organization or the block is not
    /// loaded, or changes are unsaved) is held until the organizations, the graph or the
    /// unsaved changes change, and dropped after `pressed_notifications::OPENING_PATIENCE`.
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    fn open_pressed_notifications(&mut self, changes: &be_host_stack::Changes) {
        let waiting = |host: &Self| {
            (
                host.organization
                    .as_ref()
                    .map(|organization| organization.id),
                host.organizations.len(),
                be_host_stack::status().unsaved,
            )
        };
        if !pressed_notifications::has_openings()
            || (!changes.graph && self.pressed_waiting_on == Some(waiting(self)))
        {
            return;
        }
        self.pressed_waiting_on = None;
        for opening in pressed_notifications::take_openings() {
            if opening.pressed.elapsed() > pressed_notifications::OPENING_PATIENCE {
                continue;
            }
            let now = waiting(self);
            if now.0 != Some(opening.organization) {
                // A block of another organization opens it first, unless that would discard
                // changes not saved yet, which the organization menu asks about instead: it
                // waits for them to be saved.
                let other = self
                    .organizations
                    .iter()
                    .find(|organization| organization.id == opening.organization)
                    .cloned();
                match other {
                    Some(other) if now.2 == 0 => {
                        self.open_organization(other);
                        pressed_notifications::hold(opening);
                    }
                    // The organizations are listed and it is not among them: the person
                    // is not a member, so there is nothing to open.
                    None if !self.organizations.is_empty() => {}
                    _ => {
                        pressed_notifications::hold(opening);
                        self.pressed_waiting_on = Some(now);
                    }
                }
                continue;
            }
            let Some(node) = be_host_stack::node(opening.block) else {
                pressed_notifications::hold(opening);
                self.pressed_waiting_on = Some(now);
                continue;
            };
            self.show_block(
                None,
                BlockOpen {
                    id: opening.block,
                    block_type: node.content_type,
                    via: None,
                    editor: None,
                    at: None,
                },
            );
        }
    }

    /// Registers this device's push endpoint for each organization as it is opened, once it
    /// connects (`bePush.sync` in a browser): the person turned pushes on in one, and an
    /// endpoint the push service replaced is found here. On Android, a new endpoint from the
    /// distributor registers again.
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    fn sync_push_subscription(&mut self) {
        #[cfg(target_os = "android")]
        if android_push::changed() {
            self.push_synced = None;
        }
        let Some(organization) = self
            .organization
            .as_ref()
            .map(|organization| organization.id)
        else {
            return;
        };
        if self.push_synced == Some(organization)
            || !be_host_stack::status().connected
            || be_host_base::push_device::state() != be_host_base::push_device::PushDevice::On
        {
            return;
        }
        self.push_synced = Some(organization);
        #[cfg(target_arch = "wasm32")]
        web_push::sync();
        #[cfg(target_os = "android")]
        android_push::sync(&self.host_state);
    }

    fn show_block(&mut self, from: Option<Uuid>, open: BlockOpen) {
        self.block_types.insert(open.id, open.block_type);
        self.reveals.opened(open.id, open.at.clone(), host::now());
        if let Some(handler) = self.handler_for(from, ShellRequest::ShowBlock) {
            self.with_editor(handler, |handler| handler.show_block(open));
        }
    }

    fn close_editor(&mut self, id: Uuid) {
        if self.shell == Some(id) {
            return;
        }
        self.reveals.closed(id);
        be_host_desktop::programs::close(id);
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

    fn with_editor<R>(&self, id: Uuid, act: impl FnOnce(&mut AppEditor) -> R) -> Option<R> {
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
                        <be_block::SettingsProfileContent as be_block_content::BlockContent>::CONTENT_TYPE,
                    via: None,
                    editor: None,
                    at: None,
                },
            );
        }
        let hidden = be_host_desktop::programs::bound_windows();
        if Some((Host::revision(), hidden.clone())) != self.windows_sent {
            let listed = Host::listed(&hidden);
            be_host_base::watches::publish::<be_sdk_api::OpenProgramsWatch>(&open_programs(
                &listed,
            ));
            be_host_base::watches::publish::<be_sdk_api::WindowsWatch>(&listed);
            self.windows_sent = Some((Host::revision(), hidden));
        }
        let heard = std::mem::take(&mut self.instance_events);
        let keys: HashMap<(String, be_sdk_api::EditorInstanceId), Uuid> = match heard.is_empty() {
            true => HashMap::new(),
            false => self.editors.with(|open| {
                open.iter()
                    .filter_map(|(key, editor)| {
                        let source = editor.source()?;
                        Some(((source.app_id, source.instance), *key))
                    })
                    .collect()
            }),
        };
        let mut refocused = false;
        let mut rewatched = false;
        let mut children = Vec::new();
        for heard in heard {
            let key = keys.get(&(heard.app_id.clone(), heard.instance)).copied();
            match (heard.event, key) {
                (be_host_runtime::InstanceEvent::Focused(focus), Some(id)) => {
                    self.focus_reports.insert(
                        id,
                        editors::FocusReport {
                            block: focus.block,
                            editor: focus.editor,
                            via: focus.via,
                        },
                    );
                    refocused = true;
                }
                (be_host_runtime::InstanceEvent::ArtifactWatch(watch), Some(id)) => {
                    self.artifact_watches.insert(id, watch);
                    rewatched = true;
                }
                (
                    event @ (be_host_runtime::InstanceEvent::ChildCommitted(_)
                    | be_host_runtime::InstanceEvent::ChildDismissed(_)
                    | be_host_runtime::InstanceEvent::ChildClosed(_)),
                    _,
                ) => children.push(be_host_runtime::Heard {
                    app_id: heard.app_id,
                    instance: heard.instance,
                    event,
                }),
                _ => {}
            }
        }
        if refocused && let Some(focus) = self.focus(shell) {
            be_host_runtime::set_focus(focus.block, focus.editor, focus.via);
        }
        if rewatched {
            self.watch_artifacts(self.watched());
        }
        self.forward_child_commits(children);
        self.editors.step_creations();
        for (from, action) in compositor::take_actions() {
            self.handle_editor_action(from, action);
        }
        self.reveal_opened();
    }

    /// Hands each block opened at a place (a search hit's) that place, once its editor is open.
    fn reveal_opened(&mut self) {
        if self.reveals.is_empty() {
            return;
        }
        let editors = self.editors.clone();
        self.reveals.deliver(host::now(), |block, at| {
            editors.with(|open| match open.get(&block) {
                Some(editor) if editor.reveal(at.clone()) => reveals::Delivery::Told,
                Some(_) => reveals::Delivery::Opening,
                None => reveals::Delivery::NoEditor,
            })
        });
    }

    fn focus(&self, shell: Uuid) -> Option<editors::FocusReport> {
        let mut report = self.focus_reports.get(&shell)?;
        let mut at = shell;
        let mut visited = HashSet::from([shell]);
        while let Some((block, _)) = report.block
            && let inside = self.placed_in(at, block)
            && visited.insert(inside)
            && let Some(inner) = self.focus_reports.get(&inside)
            && inner.block.is_some()
        {
            report = inner;
            at = inside;
        }
        Some(editors::FocusReport {
            block: report.block,
            editor: report.editor,
            via: report.via.clone(),
        })
    }

    /// The key of the editor showing `block` inside the editor keyed `parent`: a second
    /// placement of a block has a key of its own (`Editors::block`).
    fn placed_in(&self, parent: Uuid, block: Uuid) -> Uuid {
        self.focus_reports
            .keys()
            .copied()
            .find(|key| {
                *key != block
                    && self.editors.block(*key) == block
                    && self.editors.parent_of(*key) == Some(parent)
            })
            .unwrap_or(block)
    }

    fn watched(&self) -> Vec<Uuid> {
        let mut watched: Vec<Uuid> = self.artifact_watches.values().flatten().copied().collect();
        watched.sort_unstable();
        watched.dedup();
        watched
    }

    /// Acts on what instances' children asked: commits, dismissals and closes.
    fn forward_child_commits(&mut self, heard: Vec<be_host_runtime::Heard>) {
        if heard.is_empty() {
            self.editors.forget_hidden_settings();
            return;
        }
        let editors: Vec<(Uuid, InstanceSource)> = self.editors.with(|open| {
            open.iter()
                .filter_map(|(key, editor)| Some((*key, editor.source()?)))
                .collect()
        });
        let hosted = self.editors.hosted_sources();
        for (id, source) in editors.iter().chain(&hosted) {
            let mut commits = Vec::new();
            let mut dismissed = Vec::new();
            let mut closed = Vec::new();
            for heard in heard
                .iter()
                .filter(|heard| heard.from(&source.app_id, source.instance))
            {
                match &heard.event {
                    be_host_runtime::InstanceEvent::ChildCommitted(commit) => {
                        commits.push(commit.clone());
                    }
                    be_host_runtime::InstanceEvent::ChildDismissed(child) => dismissed.push(*child),
                    be_host_runtime::InstanceEvent::ChildClosed(child) => closed.push(*child),
                    _ => {}
                }
            }
            let (settings, commits): (Vec<_>, Vec<_>) = commits
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
            let (settings, dismissed): (Vec<_>, Vec<_>) = dismissed
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
            if !closed.is_empty() {
                self.editors.close_children(*id, &closed);
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
            let mut state = be_sdk_api::ArtifactState {
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
            Some(be_block_content::Artifact {
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
            EditorAction::Command { id, command } => self.handle_block_command(from, id, command),
            EditorAction::SimulateAccess { id, access } => self.simulate_access(id, access),
        }
    }

    fn close_itself(&mut self, id: Uuid) {
        if self.shell != Some(id) {
            let parent = self.editors.parent_of(id);
            if let Some(handler) = self.handler_for(parent, ShellRequest::Dialog)
                && handler != id
            {
                self.with_editor(handler, |handler| {
                    handler.show_dialog(id, ShellDialog::Close)
                });
            }
            return;
        }
        #[cfg(target_os = "linux")]
        if self.login_session.is_some() {
            self.request_power(
                be_sdk_api::PowerAction::LogOut,
                login_session::Trigger::Menu,
            );
            return;
        }
        ui::send(ui::UiCommand::CloseApp);
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

    fn handle_block_command(&mut self, from: Option<Uuid>, id: Uuid, command: BlockCommand) {
        match command {
            BlockCommand::Share => self.show_dialog(from, id, ShellDialog::Share),
            BlockCommand::Leave => ask_to_leave(&mut self.leaving, id),
            BlockCommand::CloseEditor if from == Some(id) => self.close_itself(id),
            BlockCommand::CloseEditor => {
                if may_close(from, self.shell, self.editors.parent_of(id)) {
                    self.close_editor(id);
                }
            }
            // The runtime does these itself (`Instances::act`).
            BlockCommand::Version(_) | BlockCommand::SetParent(_) | BlockCommand::Define { .. } => {
            }
            BlockCommand::Undo if self.editor_access(id).can_edit() => be_host_stack::undo(id),
            BlockCommand::Redo if self.editor_access(id).can_edit() => be_host_stack::redo(id),
            BlockCommand::Undo | BlockCommand::Redo => {}
            BlockCommand::History(commit) => {
                if let Some(shell) = self.shell {
                    self.with_editor(shell, |shell| shell.show_history(id, commit));
                }
            }
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
            BlockCommand::Reorder { parent, after } => {
                self.queue_reorder(id, Uuid::from_bytes(parent), after.map(Uuid::from_bytes))
            }
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

    fn frame(&mut self) {
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
                    panic_guard::take().unwrap_or_else(|| "be-host stopped responding.".into()),
                );
            }
        }
        self.run_display_prompt();
        #[cfg(target_os = "linux")]
        self.run_screen_lock();
    }

    fn run_display_prompt(&mut self) {
        let now = host::now();
        let mut prompt = display::prompt(now);
        if prompt == display::Prompt::Expired {
            let edits = display::revert(&self.host_state, &self.display.layers());
            self.display.commit(&self.host_state, edits);
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

    /// Moves the theme setting on from the system's to light, dark and back, written to the
    /// layer the Settings app would write it to; `sync_ui_settings` applies it as it applies
    /// a change made there.
    fn cycle_theme(&mut self) {
        let setting = DisplaySettings::theme_setting();
        let this_device = self.display.layers().this_device.unwrap_or_default();
        let layer = setting.layer(&this_device);
        let next = match self
            .display
            .applied()
            .and_then(|settings| setting.get(settings))
        {
            None => Some(beui::ThemeKind::Light),
            Some(beui::ThemeKind::Light) => Some(beui::ThemeKind::Dark),
            Some(_) => None,
        };
        let edit: local_settings::LayerEdit<DisplaySettings> =
            std::rc::Rc::new(move |own: &DisplaySettings| setting.set(own, next));
        self.display.commit(&self.host_state, vec![(layer, edit)]);
    }

    fn answer_display(&mut self, round: u64, keep: bool) {
        let display::Prompt::Asking { round: asked, .. } = display::prompt(host::now()) else {
            return;
        };
        if asked != round {
            return;
        }
        let edits = match keep {
            true => display::keep(&self.host_state, &self.display.layers()),
            false => display::revert(&self.host_state, &self.display.layers()),
        };
        self.display.commit(&self.host_state, edits);
        self.display_round = None;
    }

    fn crashed(&mut self, report: String) {
        let _ = self.host_state.clear_active_account();
        #[cfg(target_os = "linux")]
        if self.login_session.is_some() {
            panic_guard::persist(&report);
            std::panic::resume_unwind(Box::new(panic_guard::summary(&report).to_owned()));
        }
        panic_guard::forget();
        self.error = Some(report);
    }

    fn crashed_before(&mut self, report: String) {
        #[cfg(target_os = "linux")]
        if self.login_session.is_some() {
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
            "be-host closed after this error the last time it ran.\n\n{report}"
        ));
    }

    fn instance_heard(&mut self, heard: be_host_runtime::Heard) {
        use be_host_runtime::InstanceEvent::{
            ArtifactWatch, ChildClosed, ChildCommitted, ChildDismissed, Focused,
        };
        if matches!(
            heard.event,
            Focused(_) | ArtifactWatch(_) | ChildCommitted(_) | ChildDismissed(_) | ChildClosed(_)
        ) {
            self.instance_events.push(heard);
        }
    }

    fn run_frame(&mut self) {
        performance::begin_frame();
        be_host_runtime::poll();
        self.watch_shell();
        desk().deliver(self);
        #[cfg(target_os = "linux")]
        self.run_notifications();
        self.warm_apps();
        if !self.signed_in {
            be_host_stack::stop();
            performance::end_frame();
            return;
        }
        if self.scheduled_organization_list {
            self.scheduled_organization_list = false;
            let mut account = self.account.clone();
            account.last_organization_id = None;
            let _ = self.host_state.set_last_organization(&account, None);
            self.switch_account(account);
        }
        if self.organization.is_none() {
            be_host_stack::stop();
            self.load_organizations_if_needed();
            performance::end_frame();
            return;
        }
        if self.locked() {
            be_host_stack::stop();
            if !self.organizations_loaded {
                self.load_organizations_if_needed();
            }
            performance::end_frame();
            return;
        }
        let changes = self.stack_changes.take();
        self.root_settings.heard(&changes);
        self.input.heard(&changes);
        self.display.heard(&changes);
        self.sync_local_settings();
        self.programs.frame();
        self.sync_be_stack();
        self.notice_reconnect();
        let ready = self.organization.as_ref().is_some_and(|organization| {
            be_host_stack::installed_for(self.account.id, organization.id)
        });
        self.contacts
            .poll(&accounts::link_base(&self.server_url), ready);
        if changes.contacts {
            self.contacts.publish();
        }
        #[cfg(any(target_arch = "wasm32", target_os = "android"))]
        {
            self.open_pressed_notifications(&changes);
            self.sync_push_subscription();
        }
        self.publish_storage(changes.storage);
        if changes.notify_modes {
            self.publish_notify_modes();
        }
        self.publish_push_device();
        self.load_organizations_if_needed();
        if self.choosing_session {
            self.root_settings.ensure();
            performance::end_frame();
            return;
        }
        self.process_pending_transfers();
        self.process_pending_reorders();
        self.process_pending_copies();
        self.show_shell();
        Host::frame();
        #[cfg(target_os = "linux")]
        {
            self.take_power_request();
            be_host_desktop::remap::frame(&self.host_state);
        }
        self.poll_artifacts();
        self.record_apps_in_use();
        be_host_runtime::flush();
        performance::end_frame();
    }

    fn warm_apps(&mut self) {
        let opening = self.signed_in || self.pending_account_request.is_some();
        if !opening || std::mem::replace(&mut self.warmed, true) {
            return;
        }
        let shell = self.root_settings.shell().into_bytes();
        let in_use = self.host_state.warm_apps().unwrap_or_default();
        let manifests = be_host_stack::discovery::manifests();
        let opened: Vec<[u8; 16]> = manifests
            .iter()
            .flat_map(|manifest| manifest.editor(shell))
            .flat_map(|editor| editor.opens.iter().copied())
            .chain([shell])
            .collect();
        for manifest in manifests {
            let shown = manifest
                .editors
                .iter()
                .any(|editor| opened.contains(&editor.id));
            if shown || in_use.contains(&manifest.identity.id) {
                be_host_runtime::warm(&manifest);
            }
        }
        self.apps_in_use = in_use;
    }

    fn record_apps_in_use(&mut self) {
        let in_use = be_host_runtime::in_use();
        if in_use.is_empty() || in_use == self.apps_in_use {
            return;
        }
        if let Err(error) = self.host_state.set_warm_apps(&in_use) {
            eprintln!("be-host: the apps in use were not saved: {error}");
        }
        self.apps_in_use = in_use;
    }

    fn sync_be_stack(&mut self) {
        // A refusal goes with its request, however the request went (approved elsewhere, gone).
        if !self.pairing_refused.is_empty() {
            let open: HashSet<u64> = be_host_stack::pairing_requests()
                .into_iter()
                .map(|request| request.from)
                .collect();
            self.pairing_refused.retain(|from, _| open.contains(from));
        }
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
        if be_host_stack::installed_for(self.account.id, organization) {
            return;
        }
        let link_base = if self.server_url.is_empty() {
            String::new()
        } else {
            accounts::link_base(&self.server_url)
        };
        be_host_base::watches::publish::<be_sdk_api::LinkBaseWatch>(&link_base);
        self.publish_administrator();
        be_host_stack::start(be_host_stack::Config {
            local: self.account.server == ServerLocation::Local,
            server_url: self.server_url.clone(),
            token: self.account.token.clone(),
            account: self.account.id,
            organization,
            identity,
            display_name: self.account.name.clone(),
            email: self.account.email.clone(),
            data_dir: self.data_dir.clone(),
            clock: be_block_client::Clock::real(),
        });
    }

    /// Follows the account's storage while an editor watches it, and publishes each answer.
    fn publish_storage(&mut self, changed: bool) {
        be_host_stack::watch_storage(be_host_base::watches::watched::<
            be_sdk_api::StorageUsageWatch,
        >());
        if !changed {
            return;
        }
        let usage = be_host_stack::storage_usage().map(|usage| be_sdk_api::AccountStorage {
            held: usage.held,
            pending: usage.pending,
            quota: usage.quota,
        });
        be_host_base::watches::publish::<be_sdk_api::StorageUsageWatch>(&usage);
    }

    /// Publishes the account's notification modes, which the host stack read anew.
    fn publish_notify_modes(&mut self) {
        let modes = be_host_stack::notify_modes().map(|modes| {
            modes
                .into_iter()
                .map(|(block, mode)| be_sdk_api::BlockNotifyMode {
                    block: block.into_bytes(),
                    mode: be_host_stack::app_graph::notify_mode_to(mode),
                })
                .collect::<Vec<_>>()
        });
        be_host_base::watches::publish::<be_sdk_api::NotifyModesWatch>(&modes);
    }

    /// Publishes whether this device shows pushes whenever it changed.
    fn publish_push_device(&mut self) {
        let revision = Some(be_host_base::push_device::revision());
        if self.push_device_published == revision {
            return;
        }
        self.push_device_published = revision;
        be_host_base::watches::publish::<be_sdk_api::PushDeviceWatch>(
            &match be_host_base::push_device::state() {
                be_host_base::push_device::PushDevice::Unsupported => None,
                be_host_base::push_device::PushDevice::Off => Some(be_sdk_api::PushDevice::Off),
                be_host_base::push_device::PushDevice::On => Some(be_sdk_api::PushDevice::On),
                be_host_base::push_device::PushDevice::Denied => {
                    Some(be_sdk_api::PushDevice::Denied)
                }
            },
        );
    }

    fn publish_administrator(&self) {
        be_host_base::watches::publish::<be_sdk_api::AdministratorWatch>(
            &self.organization.as_ref().is_some_and(|organization| {
                organization.kind == be_block_protocol::OrganizationKind::Company
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
                .filter(|organization| {
                    organization.kind == be_block_protocol::OrganizationKind::Company
                })
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
        self.input.sync(&self.host_state);
        self.display.sync(&self.host_state);
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
                    self.begin_account_request(form, None);
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
            UiCommand::CheckLoginNow => {
                if let Some(pending) = &self.pending_account_request {
                    let _ = pending.check_now.send(());
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
                if let Err(error) = self.host_state.clear_active_account() {
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
            UiCommand::ConfirmRemoveAccount => {
                if let Some(account) = self.removing_account.take() {
                    self.forget_account(&account);
                }
            }
            UiCommand::CancelRemoveAccount => self.removing_account = None,
            UiCommand::OpenInspector => self.inspector_requested = Some(true),
            UiCommand::InviteMember => {
                if self.organization.is_some() {
                    self.invitations.invite();
                }
            }
            UiCommand::OpenInvitations => self.invitations.show(),
            UiCommand::CloseInvitations => self.invitations.close(),
            UiCommand::RevokeInvitation(link) => self.invitations.revoke(link),
            UiCommand::SwitchOrganization => {
                if be_host_stack::flush() == 0 {
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
            UiCommand::RestoreUnsent(block) => be_host_stack::restore_unsent(block),
            UiCommand::DiscardUnsent(block) => self.discarding_unsent = Some(block),
            UiCommand::ConfirmDiscardUnsent => {
                if let Some(block) = self.discarding_unsent.take() {
                    be_host_stack::discard_unsent(block);
                }
            }
            UiCommand::CancelDiscardUnsent => self.discarding_unsent = None,
            UiCommand::DismissToast(id) => {
                if let Some(account) = notices::dismiss(id) {
                    let dismissing = accounts::dismiss_account_notice(
                        self.server_url.clone(),
                        self.account.token.clone(),
                        account,
                    );
                    platform::spawn_request(async move {
                        let _ = dismissing.await;
                    });
                }
            }
            UiCommand::CycleTheme => self.cycle_theme(),
            UiCommand::KeepDisplay(round) => self.answer_display(round, true),
            UiCommand::RevertDisplay(round) => self.answer_display(round, false),
            #[cfg(target_os = "linux")]
            UiCommand::LockScreen => {
                if let Some(lock) = &mut self.screen_lock {
                    lock.lock(login_session::Trigger::Shortcut);
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
                    .is_some_and(login_session::ScreenLock::locked)
                {
                    self.request_power(action, login_session::Trigger::Menu);
                }
            }
            #[cfg(not(target_os = "linux"))]
            UiCommand::LockScreen | UiCommand::Unlock(_) | UiCommand::LockPower(_) => {}
            UiCommand::CloseInvite => self.invitations.close_invite(),
            UiCommand::OpenAgents => {
                self.agents_open = true;
                self.agent_created = None;
                if self.organization.is_some() {
                    self.begin_organization_request(OrganizationOperation::ListAgents);
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
                if self.organization.is_some() && !name.trim().is_empty() {
                    self.begin_organization_request(OrganizationOperation::CreateAgent(name));
                }
            }
            UiCommand::RemoveAgent(account) => {
                if self.organization.is_some() {
                    self.begin_organization_request(OrganizationOperation::RemoveAgent(account));
                }
            }
            UiCommand::Discard => self.discard(context),
            UiCommand::CancelDiscard => self.pending_destructive_action = None,
            UiCommand::ConfirmLeave(shown) => {
                if let Some(block) = confirmed_leave(self.leaving, shown)
                    && self.leave_answers.is_none()
                {
                    let (answers, post) = desk().mailbox(HostCore::left);
                    be_host_stack::leave(block, move |left| post.send(left));
                    self.leave_answers = Some(answers);
                    self.leave_refused = None;
                }
            }
            UiCommand::CancelLeave => {
                self.leaving = None;
                self.leave_answers = None;
                self.leave_refused = None;
            }
            UiCommand::Debug(command) => debug::command(command),
            UiCommand::ShowPanel(panel) => self.show_panel(panel),
            UiCommand::ConfirmRecovery(words) => {
                self.keys.confirm_recovery(
                    &words,
                    self.server_url.clone(),
                    self.account.token.clone(),
                    self.identity,
                    Self::recovery_saved(),
                );
            }
            UiCommand::UnlockWithPhrase(phrase) => {
                if let Some(secret) = self.keys.unlock_with_phrase(&phrase) {
                    if let Err(error) = self.host_state.set_identity(&self.account, secret) {
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
                    Self::paired(),
                );
            }
            UiCommand::CancelPairing => self.keys.cancel_pairing(),
            UiCommand::NewRecoveryPhrase => self.keys.replace_recovery(),
            UiCommand::CancelRecovery => self.keys.cancel_replacing(),
            UiCommand::ApprovePairing(from, code) => {
                self.pairing_refused.remove(&from);
                let (answers, post) = desk().mailbox(move |core: &mut HostCore, approved| {
                    core.pairing_approved(from, approved)
                });
                be_host_stack::approve_pairing(from, code, move |approved| post.send(approved));
                self.pairing_answers.insert(from, answers);
            }
            UiCommand::DismissPairing(from) => {
                self.pairing_refused.remove(&from);
                self.pairing_answers.remove(&from);
                be_host_stack::dismiss_pairing(from);
            }
        }
    }

    fn view(&self) -> HostUi {
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
        let be_status = be_host_stack::status();
        let sync = match screen {
            ui::Screen::Organization => sync_state(&be_status),
            _ => ui::SyncState::Saved,
        };
        let listed = matches!(screen, ui::Screen::Accounts | ui::Screen::Organizations);
        let companies = |account: &SavedAccount| -> Vec<String> {
            if !listed {
                return Vec::new();
            }
            let url = account.server.url();
            self.workspaces
                .workspaces(&url, account.id)
                .into_iter()
                .filter(|organization| {
                    organization.kind == be_block_protocol::OrganizationKind::Company
                })
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
        HostUi {
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
                ui::Screen::Organization => be_host_stack::pairing_requests()
                    .into_iter()
                    .map(|request| ui::PairingRow {
                        from: request.from,
                        refused: self
                            .pairing_refused
                            .get(&request.from)
                            .cloned()
                            .unwrap_or_default(),
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
                joining: self.joining.as_ref().and_then(joining_row),
                busy: self.pending_organization_request.is_some(),
                error: self.organization_error.clone(),
                created: self.organization_created,
            },
            removing_account: self
                .removing_account
                .as_ref()
                .map(|account| account.name.clone()),
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
                    organization.kind == be_block_protocol::OrganizationKind::Company
                }),
                administrator: self.organization.as_ref().is_some_and(|organization| {
                    organization.kind == be_block_protocol::OrganizationKind::Company
                        && organization.role == OrganizationRole::Administrator
                }),
                escrow_warning: be_host_stack::escrow_warning(),
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
                created: self.agent_created.clone(),
            }),
            contacts: self.contacts.view(),
            members: self.members.view(self.account.id),
            invite: self.invitations.invite_view(organization_name),
            invitations: self.invitations.view(),
            about: self.about_open,
            unsent: self.unsent_open.then(|| ui::UnsentView {
                rows: be_host_stack::unsent()
                    .into_iter()
                    .map(|unsent| ui::UnsentRow {
                        block: unsent.block,
                        name: unsent.name.unwrap_or_else(|| "Untitled".to_owned()),
                        offerable: unsent.offerable,
                        refused: unsent.refused,
                        kept_edits: unsent.kept_edits,
                    })
                    .collect(),
                discarding: self.discarding_unsent,
            }),
            discard: self.pending_destructive_action.as_ref().map(discard_view),
            leaving: self.leaving.map(leaving_view),
            leave_busy: self.leave_answers.is_some(),
            leave_refused: self.leave_refused.clone().unwrap_or_default(),
            presenting: self
                .editors
                .with(|open| open.values().any(AppEditor::presenting_now)),
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
        be_host_stack::stop();
        match Self::new(Some(self.data_dir.clone())) {
            Ok(mut fresh) => {
                fresh.root_settings = RootSettings::new(self.root_settings.shell());
                #[cfg(target_os = "linux")]
                {
                    fresh.login_session = self.login_session.take();
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

    /// Starts again from the host state the page holds, which is what its worker keeps.
    #[cfg(target_arch = "wasm32")]
    fn restart(&mut self) {
        be_host_stack::stop();
        let host_state = std::mem::replace(&mut self.host_state, HostStateStore::placeholder());
        let workspaces = std::mem::take(&mut self.workspaces);
        match Self::new(host_state, workspaces, self.host_files_failed.take()) {
            Ok(fresh) => *self = fresh,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Has the host's worker delete the host state, and starts again without it.
    #[cfg(target_arch = "wasm32")]
    fn delete_client_database(&mut self) {
        match self.workspaces.forget_host_state() {
            Some(fresh) => {
                self.host_state = fresh;
                self.restart();
            }
            None => self.error = Some("the host's worker is gone".to_owned()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn delete_client_database(&mut self) {
        let path = self.data_dir.join("host.sqlite3");
        let _ = std::mem::replace(&mut self.host_state, HostStateStore::placeholder());
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            self.error = Some(format!("failed to delete {}: {error}", path.display()));
            return;
        }
        self.restart();
    }
}

fn sync_state(status: &be_host_stack::Status) -> ui::SyncState {
    // A local account has no server: saved on this device is saved.
    match (&status.error, status.connected || status.local) {
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
            "Closing be-host will discard changes that are not saved on this device yet. Changes waiting to sync are kept."
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

fn artifact_of(id: Uuid) -> Option<be_block_content::Artifact> {
    be_host_stack::node(id)?
        .metadata
        .get(&be_block_content::ARTIFACT)
}

fn set_artifact(id: Uuid, artifact: Option<be_block_content::Artifact>) {
    be_host_stack::define(
        id,
        vec![be_block_content::ARTIFACT.definition(artifact.as_ref())],
    );
}

/// What Restore knows of a deleted block: where it was, and what it was deleted from when its
/// parent no longer says (`DELETED_FROM`).
struct Restoring {
    parent: Option<BlockParent>,
    deleted_from: Option<be_block_content::DeletedFrom>,
    /// The account's Unfiled, once the graph has it at the root.
    unfiled: Option<Uuid>,
    /// Whether the block itself may stand at the root (a workspace).
    root_addable: bool,
}

#[derive(Debug, PartialEq)]
enum RestoreTo {
    /// Its parent still holds it in place: list it there again.
    Placed(Uuid),
    Root,
    /// Into the account's Unfiled, which lists nothing, so setting the parent is enough.
    Unfiled(Uuid),
    /// Into a block that must list it.
    Into(Uuid),
    Nowhere,
}

/// Where Restore puts a deleted block back: where it was deleted from when that still takes
/// it, else into the account's Unfiled.
fn restore_to(
    restoring: &Restoring,
    usable: impl Fn(Uuid) -> bool,
    addable: impl Fn(Uuid) -> bool,
) -> RestoreTo {
    if let Some(BlockParent::Block(parent)) = restoring.parent
        && usable(parent)
        && addable(parent)
    {
        return RestoreTo::Placed(parent);
    }
    let from = match restoring.parent {
        Some(BlockParent::Block(parent)) => Some(be_block_content::DeletedFrom::Block(parent)),
        _ => restoring.deleted_from,
    };
    match from {
        Some(be_block_content::DeletedFrom::Block(parent))
            if restoring.unfiled == Some(parent) && usable(parent) =>
        {
            RestoreTo::Unfiled(parent)
        }
        Some(be_block_content::DeletedFrom::Root) if restoring.root_addable => RestoreTo::Root,
        Some(be_block_content::DeletedFrom::Block(parent)) if usable(parent) && addable(parent) => {
            RestoreTo::Into(parent)
        }
        Some(be_block_content::DeletedFrom::Root | be_block_content::DeletedFrom::Block(_))
        | None => restoring
            .unfiled
            .map_or(RestoreTo::Nowhere, RestoreTo::Unfiled),
    }
}

/// Whether `block` is an account's Unfiled, which lists nothing: a block goes into it by its
/// parent alone.
fn is_unfiled(block: Uuid) -> bool {
    be_host_stack::node(block).is_some_and(|node| node.unfiled())
}

/// Says that `container` keeps linking to the block an unlink would have copied: its type
/// cannot put a copy in the block's place, or the container can't be changed right now.
fn name_of(block: Uuid, otherwise: &str) -> String {
    be_host_stack::node(block)
        .and_then(|node| node.metadata.name())
        .unwrap_or_else(|| otherwise.to_owned())
}

/// A container's type refused to let the block called `child` out (`into` false) or to take it
/// in.
fn refuse_transfer(container: Uuid, child: &str, into: bool) {
    let container = name_of(container, "That block");
    notices::refuse(match into {
        true => format!("{container} can't hold {child}, so it stays where it was."),
        false => format!("{container} can't let go of {child}, so it stays where it was."),
    });
}

/// The sibling `child` follows in `container`'s content (none: it is first), or none when the
/// container does not hold it.
fn sibling_before(container: Uuid, child: Uuid) -> Option<Option<Uuid>> {
    let contents = be_host_stack::node(container)?.contents();
    let at = contents.iter().position(|held| *held == child)?;
    Some(at.checked_sub(1).map(|before| contents[before]))
}

/// A source that let a block go refused to take it back after its destination refused it.
fn refuse_put_back(source: Uuid, child: &str) {
    let source = name_of(source, "Its folder");
    notices::refuse(format!("{source} couldn't take {child} back either."));
}

/// A container's type refused to put `child` somewhere else in its order.
fn refuse_reorder(container: Uuid, child: Uuid) {
    let (container, child) = (
        name_of(container, "That block"),
        name_of(child, "that block"),
    );
    notices::refuse(format!(
        "{container} can't move {child} within it, so it stays where it was."
    ));
}

fn refuse_copy(container: Uuid) {
    let name = name_of(container, "That block");
    notices::refuse(format!(
        "{name} can't take its own copy of that block, so it still links to the original."
    ));
}

/// Offline, a block goes into, out of or along a container by changing the container's content,
/// which this device must hold: refuses, saying why, when it holds none
/// (`be_host_stack::out_of_reach`).
fn refused_offline(container: Uuid) -> bool {
    if is_unfiled(container) || !be_host_stack::out_of_reach(container) {
        return false;
    }
    let name = be_host_stack::node(container)
        .and_then(|node| node.metadata.name())
        .unwrap_or_else(|| "That folder".to_owned());
    notices::refuse(format!(
        "{name} hasn't been opened on this device, so it can't be changed while you're offline. \
         Try again once you're back online."
    ));
    true
}

fn drag_source(location: BlockLocation) -> SidebarDragSource {
    match location {
        BlockLocation::Root => SidebarDragSource::Root,
        BlockLocation::Detached => SidebarDragSource::Orphaned,
        BlockLocation::Unfiled => SidebarDragSource::Unfiled,
        BlockLocation::Block(id) if is_unfiled(Uuid::from_bytes(id)) => SidebarDragSource::Unfiled,
        BlockLocation::Block(id) => SidebarDragSource::Block(Uuid::from_bytes(id)),
    }
}

/// An editor asked to leave `block`: the dialog asks about it, unless it already asks about
/// another, which a second ask never swaps out from under the person.
fn ask_to_leave(pending: &mut Option<Uuid>, block: Uuid) {
    if pending.is_none() {
        *pending = Some(block);
    }
}

/// The block a confirm in the leave dialog leaves: the one it showed, and only while that is
/// still the one pending.
fn confirmed_leave(pending: Option<Uuid>, shown: Uuid) -> Option<Uuid> {
    pending.filter(|pending| *pending == shown)
}

/// The leave dialog's view of `block`, named from the host's graph.
fn leaving_view(block: Uuid) -> ui::LeavingView {
    let node = be_host_stack::node(block);
    let name = node
        .as_ref()
        .and_then(|node| node.metadata.name())
        .unwrap_or_else(|| "Untitled".to_owned());
    let within = node
        .and_then(|node| node.parent?.block())
        .and_then(be_host_stack::node)
        .and_then(|parent| parent.metadata.name());
    ui::LeavingView {
        block,
        name,
        within,
    }
}

/// Whether the editor showing `from` may close another editor, whose placer is `parent`: the
/// shell closes any (a tab), and an editor only a child it placed, never one elsewhere.
fn may_close(from: Option<Uuid>, shell: Option<Uuid>, parent: Option<Uuid>) -> bool {
    from.is_some() && (from == shell || parent == from)
}

#[cfg(test)]
mod tests;

/// The programs of the listed windows, each once, for apps other than the shell.
fn open_programs(listed: &[be_sdk_api::HostWindow]) -> Vec<be_sdk_api::OpenProgram> {
    let mut programs: Vec<be_sdk_api::OpenProgram> = Vec::new();
    for window in listed {
        if window.kind != be_sdk_api::WindowKind::Window {
            continue;
        }
        let program = be_sdk_api::OpenProgram {
            program_id: window.program_id.clone(),
            command: window.origin.command.clone(),
        };
        if !programs.contains(&program) {
            programs.push(program);
        }
    }
    programs
}
