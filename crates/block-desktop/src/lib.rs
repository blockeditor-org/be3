#![cfg(target_os = "linux")]

mod dbus;
mod display;
mod input;
mod local_settings;
mod lock_screen;
mod media;
mod notifications;
mod password;
mod programs;
mod session;
mod tray;
mod wayland;

use std::cell::RefCell;
use std::error::Error;

use be_block::{DisplaySettings, InputSettings};
use beui::reactive::{List, ReadSignal, WriteSignal, create_memo, create_signal, view};
use beui::styled::KeepChanges;
use beui::{Context, Document, NodeId, Rect, Setup};
use block_app_lib::Launch;
use block_app_lib::extension::{self, Access, App, Extension, allow_action, allow_value};
use block_plugin_api::{
    Displays, HostWindows, InputDevices, Media, MediaRequest, NotificationReport, Notifications,
    Power, PowerAction, PowerAvailability, ProgramAction, Programs, ScreenLocked, Tray,
    TrayRequest, WindowAction,
};

use local_settings::SettingsSync;
use lock_screen::{LockCommand, LockView};

thread_local! {
    static DISPLAY_ANSWERS: RefCell<Vec<(u64, bool)>> = const { RefCell::new(Vec::new()) };
}

fn answer_display(round: u64, keep: bool) {
    DISPLAY_ANSWERS.with(|answers| answers.borrow_mut().push((round, keep)));
    extension::request_repaint();
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--install-session") {
        return session::install(arguments.get(1).map(String::as_str));
    }
    let on_seat = arguments.iter().any(|argument| argument == "--session");
    if on_seat {
        session::start_log();
    }
    let mut launch = Launch::new()?;
    launch.set_shell(be_block::LINUX_DESKTOP_EDITOR);
    for argument in arguments {
        match argument.as_str() {
            "--session" => {}
            "--dev-workspace" => launch.open_dev_workspace(),
            _ => return Err(format!("unknown argument {argument}").into()),
        }
    }
    allow();
    let app = launch.into_app(Desktop::new());
    let options = block_app_lib::run_options();
    if on_seat {
        return beui::run_on(Box::new(beui_adapter_drm::Drm), options, app);
    }
    let renderer = beui::WindowRenderer::Wgpu {
        open_device: Some(std::sync::Arc::new(be_dmabuf::open_device)),
    };
    beui::run_with_renderers(options, vec![renderer], app)
}

fn allow() {
    allow_value::<HostWindows>(Access::Shell);
    allow_value::<InputDevices>(Access::Anyone);
    allow_value::<Displays>(Access::Anyone);
    allow_value::<Power>(Access::Anyone);
    allow_value::<Media>(Access::Anyone);
    allow_value::<Notifications>(Access::Shell);
    allow_value::<ScreenLocked>(Access::Anyone);
    allow_value::<Programs>(Access::Shell);
    allow_value::<Tray>(Access::Shell);
    allow_action::<WindowAction>(Access::Shell);
    allow_action::<PowerAction>(Access::Shell);
    allow_action::<MediaRequest>(Access::Shell);
    allow_action::<NotificationReport>(Access::Shell);
    allow_action::<ProgramAction>(Access::Shell);
    allow_action::<TrayRequest>(Access::Shell);
}

struct Desktop {
    session: Option<session::DesktopSession>,
    lock: session::ScreenLock,
    locked: bool,
    published_power: Option<PowerAvailability>,
    media: Option<media::Media>,
    notifications: Option<notifications::Notifications>,
    tray: Option<tray::Tray>,
    programs: programs::Programs,
    windows_sent: Option<u64>,
    lock_view: Option<(LockView, WriteSignal<LockView>)>,
    input: SettingsSync<InputSettings>,
    display: SettingsSync<DisplaySettings>,
    display_round: Option<u64>,
    keep_display: Option<WriteSignal<Option<u64>>>,
}

impl Desktop {
    fn new() -> Self {
        Self {
            session: None,
            lock: session::ScreenLock::start(),
            locked: false,
            published_power: None,
            media: None,
            notifications: Some(notifications::Notifications::start()),
            tray: Some(tray::Tray::start()),
            programs: programs::Programs::default(),
            windows_sent: None,
            lock_view: None,
            input: SettingsSync::default(),
            display: SettingsSync::default(),
            display_round: None,
            keep_display: None,
        }
    }

    fn run_settings(&mut self, document: &mut Document, app: &mut App) {
        self.input.sync(app);
        self.display.sync(app);
        for (round, keep) in
            DISPLAY_ANSWERS.with(|answers| std::mem::take(&mut *answers.borrow_mut()))
        {
            let display::Prompt::Asking { round: asked, .. } = display::prompt(extension::now())
            else {
                continue;
            };
            if asked != round {
                continue;
            }
            let edits = match keep {
                true => display::keep(),
                false => display::revert(),
            };
            self.display.commit(app, edits);
        }
        let now = extension::now();
        let mut prompt = display::prompt(now);
        if prompt == display::Prompt::Expired {
            self.display.commit(app, display::revert());
            prompt = display::prompt(now);
        }
        let round = match prompt {
            display::Prompt::Asking { round, left } => {
                extension::request_repaint_after(left);
                Some(round)
            }
            display::Prompt::Settled | display::Prompt::Expired => None,
        };
        if round != self.display_round {
            self.display_round = round;
            if let Some(set) = self.keep_display.clone() {
                beui::reactive::with_reactive_scope(document, move || set.set(round));
            }
        }
    }

    fn run_windows(&mut self) {
        if Some(wayland::revision()) != self.windows_sent {
            self.windows_sent = Some(wayland::revision());
            extension::publish::<HostWindows>(&wayland::listed());
        }
        for action in extension::take_actions::<WindowAction>() {
            match action {
                WindowAction::Focus(window) => wayland::focus(window),
                WindowAction::Close(window) => wayland::close(window),
                WindowAction::Fullscreen { window, fullscreen } => {
                    wayland::fullscreen(window, fullscreen);
                }
            }
        }
    }

    fn take_power_request(&mut self) {
        let request = extension::take_actions::<PowerAction>().pop();
        if let Some(action) = request
            && self
                .published_power
                .is_some_and(|power| power.allows(action))
        {
            self.request_power(action, session::Trigger::Menu);
        }
    }

    fn request_power(&mut self, action: PowerAction, trigger: session::Trigger) {
        match action {
            PowerAction::Lock => self.lock.lock(trigger),
            action => {
                if let Some(session) = &mut self.session {
                    session.request(action);
                    extension::request_repaint();
                }
            }
        }
    }

    fn run_media(&mut self) {
        let requests = extension::take_actions::<MediaRequest>();
        if self.media.is_none() && (!requests.is_empty() || extension::watched::<Media>()) {
            self.media = Some(media::Media::start());
        }
        let Some(media) = &mut self.media else {
            return;
        };
        for request in requests {
            media.request(request);
        }
        if let Some(levels) = media.frame() {
            extension::publish::<Media>(&levels);
        }
    }

    fn run_session(&mut self, context: &Context) {
        self.lock.frame();
        if wayland::take_lock_due() {
            self.lock.lock(session::Trigger::Idle);
        }
        if let Some(session) = &mut self.session {
            session.frame(context, &mut self.lock);
        }
        let locked = self.lock.locked();
        wayland::set_locked(locked);
        extension::publish::<ScreenLocked>(&locked);
        if locked && !self.locked {
            extension::close_app_menu();
        }
        self.locked = locked;
        let mut availability = self.availability();
        availability.lock = !locked;
        if self.published_power != Some(availability) {
            self.published_power = Some(availability);
            extension::publish::<Power>(&availability);
        }
    }

    fn availability(&self) -> PowerAvailability {
        self.session
            .as_ref()
            .map(session::DesktopSession::availability)
            .unwrap_or_default()
    }

    fn show_lock(&mut self, document: &mut Document) {
        let state = self.lock.state();
        let offered = self.availability();
        let view = LockView {
            available: true,
            locked: state.locked,
            busy: state.busy,
            error: state.error,
            user: self.lock.user().to_owned(),
            power: [
                PowerAction::Suspend,
                PowerAction::Restart,
                PowerAction::PowerOff,
            ]
            .into_iter()
            .filter(|action| offered.allows(*action))
            .collect(),
        };
        if let Some((shown, set)) = &mut self.lock_view
            && *shown != view
        {
            *shown = view.clone();
            let set = set.clone();
            beui::reactive::with_reactive_scope(document, move || set.set(view));
        }
    }

    fn run_lock_commands(&mut self) {
        for command in lock_screen::take_commands() {
            match command {
                LockCommand::Lock => self.lock.lock(session::Trigger::Shortcut),
                LockCommand::Unlock(password) => self.lock.submit(password),
                LockCommand::Power(action) => {
                    if self.lock.locked() {
                        self.request_power(action, session::Trigger::Menu);
                    }
                }
            }
            extension::request_repaint();
        }
    }
}

impl Extension for Desktop {
    fn view(&mut self) -> NodeId {
        wayland::create();
        extension::show_windows_with(|window, occluders| {
            view! {
                <wayland::WindowSurface window occluders />
            }
        });
        let (lock, set_lock) = create_signal(LockView::default());
        self.lock_view = Some((LockView::default(), set_lock));
        let (keep_display, set_keep_display) = create_signal(None);
        self.keep_display = Some(set_keep_display);
        view! {
            <List spacing=0.0>
                <KeepDisplay round={keep_display} />
                <lock_screen::SessionLock lock />
                <wayland::WindowActions />
            </List>
        }
    }

    fn setup(&mut self, setup: &Setup, app: &mut App) {
        wayland::start(setup);
        input::start(setup);
        if let Some(problems) = setup.get::<beui_adapter_drm::Problems>() {
            problems.listen(extension::report);
        }
        if self.session.is_none() && session::owns_a_seat(setup) {
            self.session = Some(session::DesktopSession::start());
            session::export_environment(wayland::environment());
        }
        display::start(setup);
        self.input.boot(app);
        self.display.boot(app);
    }

    fn renderer_replaced(&mut self, setup: &Setup) {
        wayland::replace_gpu(setup);
    }

    fn before(&mut self, context: &Context, rect: Rect, document: &mut Document) {
        wayland::before(context, rect, document);
    }

    fn frame(&mut self, context: &Context, document: &mut Document, app: &mut App) {
        self.run_settings(document, app);
        if let Some(notifications) = &mut self.notifications {
            notifications.frame();
        }
        if let Some(tray) = &mut self.tray {
            tray.frame();
        }
        self.programs.frame();
        self.run_windows();
        self.take_power_request();
        self.run_media();
        self.run_session(context);
        self.show_lock(document);
    }

    fn after(&mut self, context: &Context, document: &mut Document) {
        self.lock.note_drawn(document.locked());
        wayland::after(context, document);
        self.run_lock_commands();
    }

    fn exiting(&mut self) {
        wayland::exiting();
        display::stop();
        self.media = None;
        self.notifications = None;
        self.tray = None;
        self.session = None;
    }

    fn ends_on_crash(&self) -> bool {
        self.session.is_some()
    }
}

#[beui::reactive::component]
fn KeepDisplay(round: ReadSignal<Option<u64>>) -> NodeId {
    let asking = create_memo({
        let round = round.clone();
        move || round.get().is_some()
    });
    let shown = create_memo(move || round.get().unwrap_or_default());
    let keeping = shown.clone();
    let reverting = shown.clone();
    view! {
        <KeepChanges
            open={asking}
            title="Keep these display settings?"
            timeout=display::ANSWER_WITHIN
            round={shown}
            id="display.keep"
            on_keep={move || answer_display(keeping.get_untracked(), true)}
            on_revert={move || answer_display(reverting.get_untracked(), false)}
        />
    }
}
