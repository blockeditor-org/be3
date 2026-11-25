//! The root itself, built for the apps' platform alone.

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::pin,
    rc::Rc,
    task::Poll,
    time::Duration,
};

use be_lib_geometry::pos2;
use be_lib_layers::{Area, LayerId, LayerInput, LayerList, WindowLayers};
use be_lib_ui_types::input::{Input, Key, KeyChord, KeyPress, Modifiers};
use be_sdk_api::Fonts;
use be_sdk_beui_root::{BeuiRoot, BeuiSurface};
use be_sdk_root::{Delivery, Embedding, OwnSurface};
use be_wasm_guest_abi::{Flow, Guest, Waker};
use be_wasm_ops::guest::{Executor, LocalFuture};
use be_wasm_password::Password;
use be_wasm_root_api::{
    AccountRow, AgentRow, ContactCard, ContactMismatch, ContactRow, CoreCommand, CoreSecret,
    CoreView, CoreWait, DiscardReason, DisplayQuestion, FrameDone, FrameRequest, HostStatistics,
    HostedBlock, InvitationRow, InvitationState, LockState, MemberRole, MemberRow, MemoryMinutes,
    NewAccount, Notice, OrganizationRow, OrganizationsListed, PairingApproval, PowerAction,
    ProfileRow, Refusal, RoleChange, SavingState, SecretCommand, ServerAccount, SetAside,
    ShareAccess, SharePreview, StatisticRow, SyncState, UnsentEdit, WindowSize,
    guest::{
        exports::{frames, input, paintings},
        host,
    },
};
use beui::icons::{
    Glyph, ICON_LOG_OUT, ICON_MENU, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW, ICON_TRASH,
    ICON_USER_MINUS,
};
use beui::native::painting::{Frame as PaintFrame, Painting, ScreenPaintings};
use beui::reactive::*;
use beui::styled::*;
use beui::text::Phrase;
use beui::{
    Align, Align2, Direction, Justify, KeyPhase, NodeId, Overflow, Pos2, Rect, TextAlign, vec2,
};

use crate::{
    ADD_ACCOUNT_TITLE, AGENTS_TITLE, CONTACTS_TITLE, DIALOG_LAYER, DIALOG_SURFACE, DISCARD_TITLE,
    DISCARD_UNSENT_TITLE, FAILURE_TITLE, INVITATIONS_TITLE, KEEP_DISPLAY_TITLE, LOCK_LAYER,
    LOCK_SURFACE, MEMBERS_TITLE, MENU_BUTTON, MENU_LAYER, MENU_SURFACE, ONBOARDING_LAYER,
    ONBOARDING_SURFACE, ORGANIZATIONS_TITLE, PAIRING_TITLE, PALETTE_LAYER, REAUTH_TITLE,
    RECOVERY_TITLE, REMOVE_CONTACT_TITLE, REMOVE_MEMBER_TITLE, ROOT_COLOR, ROOT_LAYER,
    ROOT_SURFACE, SESSION_NOTICE_SURFACE, SHARE_LINK_TITLE, SIGN_IN_TITLE, SLOW_SAVE,
    SLOW_SAVE_TITLE, SLOW_SAVE_TOAST, STATISTICS_TITLE, TOAST_LAYER, TOAST_SURFACE, UNSENT_TITLE,
};

mod dialogs;
mod lock;
mod menu;
mod notices;
mod onboarding;
mod palette;
mod panels;
mod session;
mod statistics;

use dialogs::*;
use lock::*;
use menu::*;
use notices::*;
use onboarding::*;
use palette::*;
use panels::*;
use session::*;
use statistics::*;

#[derive(Clone)]
struct Root {
    embedding: Rc<RefCell<Embedding>>,
    /// The fonts its host gave, which it tells the app it spawns, had on its first frame.
    fonts: Rc<RefCell<Option<Rc<Fonts>>>>,
    /// What its surfaces drawn with beui share, made on its first frame.
    beui: Rc<RefCell<Option<BeuiRoot>>>,
    /// Its own surface, filled with `ROOT_COLOR` on its first frame.
    own: Rc<RefCell<Option<OwnSurface>>>,
    /// Its host's notices as toasts, which beui draws, made on its first frame.
    toasts: Rc<RefCell<Option<BeuiSurface>>>,
    /// Its menu, which beui draws, made on its first frame: what opens the block stack's
    /// dialogs and its panel, in place of be-host's own menu.
    menu: Rc<RefCell<Option<BeuiSurface>>>,
    /// The core's view and the saving each menu drawn shows, its own and each menu panel's,
    /// which the host's answers set (`menus_show`).
    menu_shows: MenuShows,
    /// Whether its host's changes are saved, as its host said last, which a menu drawn next
    /// shows first.
    saving_now: Rc<RefCell<SavingState>>,
    /// The toasts that document shows, which the host's notices set.
    shown: Rc<Cell<Option<Signal<Vec<Toast>>>>>,
    /// Whether its host's changes are saved, as its host said last, which the toasts' document
    /// shows (the slow save's toast).
    saving: Rc<Cell<Option<Signal<SavingState>>>>,
    /// The display settings its host asks to keep, as it said last: kept while the session is
    /// locked, so the question shows once it is unlocked if the host still asks it.
    keep_asking: Rc<RefCell<Option<DisplayQuestion>>>,
    /// The round of display settings the root answered last, which it does not ask again.
    keep_answered: Rc<Cell<Option<u64>>>,
    /// What the block stack shows the person, as its host said last: the one view of it the
    /// root stores. An area's document is handed a signal of the whole view and splits off a
    /// memo of its own field, so it renders again only when that field changed; no area keeps
    /// a copy of its field apart from it.
    core: Rc<RefCell<CoreView>>,
    /// The core's view its open dialog shows, while that dialog shows the core's (the unsent
    /// edits, adding an account).
    core_shown: Rc<Cell<Option<Signal<CoreView>>>>,
    /// Its accounts, while the block stack waits for the person to open or add one.
    onboarding: Rc<RefCell<Option<Onboarding>>>,
    /// The core's view its accounts show, while they show.
    onboarding_shown: Rc<Cell<Option<Signal<CoreView>>>>,
    /// The secret its onboarding shows (the recovery phrase, a pairing code), and whether it
    /// waits for one, while it shows.
    onboarding_secret: Rc<Cell<Option<(Signal<Option<Held>>, Signal<bool>)>>>,
    /// A secret the host answered the onboarding's `make-secret` with, until the next frame
    /// hands it over, or drops it, zeroed, if the onboarding went.
    onboarding_answer: Rc<RefCell<Option<Option<CoreSecret>>>>,
    /// The pairing requests the person put aside, which the root asks about no more.
    pairing_dismissed: Rc<RefCell<Vec<u64>>>,
    /// A dialog one of its documents asked for (the accounts' add or remove), opened with the
    /// next frame unless another dialog is open.
    wanted: Rc<RefCell<Option<Asking>>>,
    /// Whether the host's question of keeping display settings replaced the dialog adding an
    /// account mid sign-in, which reopens once no dialog is open if the sign-in goes on.
    resuming: Rc<Cell<bool>>,
    /// A secret the host answered `make-secret` with (an invitation's link), until the next
    /// frame hands it to the dialog that asked, or drops it, zeroed, if that dialog closed.
    secret: Rc<RefCell<Option<Option<CoreSecret>>>>,
    /// The secret its open dialog shows (the invitations' link, the agents' credentials), and
    /// whether it waits for one, while that dialog asks for secrets: one request at a time, so
    /// an answer is the latest request's.
    secret_shown: Rc<Cell<Option<(Signal<Option<Held>>, Signal<bool>)>>>,
    /// The host's window as its last frame drew it: its width and height in points and its
    /// pixels per point, which every surface of its own but the host panels fills.
    window: Rc<Cell<Window>>,
    /// The host's time zone as its last frame said it, in seconds east of UTC, which the lock
    /// screen's clock shows the time at.
    utc_offset: Rc<Cell<i32>>,
    /// The area the toasts' document anchors them in, its whole surface, which follows the
    /// window.
    toast_area: Rc<Cell<Option<Signal<Rect>>>>,
    /// Its dialog, while it is open.
    dialog: Rc<RefCell<Option<Dialog>>>,
    /// What its input asked of the dialog, done with the next frame.
    asked: Rc<Cell<Option<Asked>>>,
    /// The host panels its apps place, each drawn on a surface of its own (`settle_panels`).
    panels: Rc<RefCell<Vec<PanelSurface>>>,
    /// How many panel surfaces it made: the next is numbered past them.
    panels_made: Rc<Cell<u32>>,
    /// Its command palette, while it is open.
    palette: Rc<RefCell<Option<Palette>>>,
    /// Whether Ctrl+Shift+P asked to open or close its palette, done with the next frame.
    palette_asked: Rc<Cell<bool>>,
    /// Whether F6 (`false`) or Shift+F6 (`true`) was pressed since the last frame, which moves
    /// the keyboard to the next app's view or the previous (`Embedding::cycle_focus`).
    cycle_asked: Rc<Cell<Option<bool>>>,
    /// The host panel its menu asked the session's editor to show, done with the next frame.
    panel_asked: Rc<Cell<Option<be_sdk_api::HostPanel>>>,
    /// How many performance panels watch the host's statistics: the host keeps them while any
    /// does.
    statistics_watchers: Rc<Cell<u32>>,
    /// Whether the feeds handing the host's statistics and its memory over time to the
    /// performance panels run (`feed_statistics`, `feed_memory`).
    statistics_fed: Rc<Cell<bool>>,
    memory_fed: Rc<Cell<bool>>,
    /// The session's screen lock as the host said last, until a frame shows it.
    lock_state: Rc<RefCell<Option<LockState>>>,
    /// Its lock screen, while the session is locked.
    lock: Rc<RefCell<Option<Locked>>>,
    /// The editor of the block the session shows, while it shows one.
    session: Rc<RefCell<Option<Session>>>,
    /// How many sessions it showed, which names the latest.
    session_rounds: Rc<Cell<u64>>,
    /// Whether the session's notice asked to start its app again, done with the next frame.
    session_reload: Rc<Cell<bool>>,
    /// The layer that had the keyboard when the root last told placers where it went
    /// (`follow_focus`).
    followed_focus: Rc<Cell<Option<LayerId>>>,
    /// The notice over the session's editor (`SessionNotice`), which beui draws, made on its
    /// first frame.
    session_notice: Rc<RefCell<Option<BeuiSurface>>>,
    /// What that notice shows, which `settle_session` sets.
    session_notice_shown: Rc<Cell<Option<Signal<Option<SessionShown>>>>>,
    /// Wakes the root, and so its host, for a frame its beui document wants.
    waker: Waker,
}

/// The host's window: its size in points and the pixels a point is drawn at.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Window {
    width: f32,
    height: f32,
    pixels_per_point: f32,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            width: 1.0,
            height: 1.0,
            pixels_per_point: 1.0,
        }
    }
}

impl Window {
    /// The window as `frame-request` tells it, at least a point each way.
    fn of(size: &WindowSize) -> Self {
        Self {
            width: size.width.max(1.0),
            height: size.height.max(1.0),
            pixels_per_point: match size.pixels_per_point > 0.0 {
                true => size.pixels_per_point,
                false => 1.0,
            },
        }
    }

    /// The pixels a surface `width` by `height` points spans in it.
    fn pixels(&self, width: f32, height: f32) -> (u32, u32) {
        let pixels = |points: f32| ((points * self.pixels_per_point).round() as u32).max(1);
        (pixels(width), pixels(height))
    }

    /// The pixels a surface filling it spans.
    fn whole(&self) -> (u32, u32) {
        self.pixels(self.width, self.height)
    }

    /// The whole window as a rect in its points, at its top left.
    fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(self.width, self.height))
    }
}

/// The root's dialog while it is open.
struct Dialog {
    surface: BeuiSurface,
    /// How the person closed it from inside, which its document sets.
    closed: Rc<RefCell<Option<Closed>>>,
    /// The round of display settings it asks to keep, if it is that question rather than the
    /// rename dialog.
    keep: Option<u64>,
    /// Whether it adds an account: closing it, unless the account was added, stops adding.
    signs_in: bool,
    /// The question of the host's it asks, if it is one: it closes once the host no longer
    /// asks it.
    host_asks: Option<HostAsks>,
    /// Whether it asks for secrets (the invitations, the agents), to which an answer is handed.
    receives_secret: bool,
    /// Rung as it closes, which stops a request for a secret it made waiting.
    closing: Closing,
    /// Whether it stays open while a request for a secret waits: one that cannot be had again
    /// (an agent's credentials), lost were it dropped.
    holds_while_pending: bool,
    /// The layer focused when it opened, which gets the focus back when it closes.
    had: Option<LayerId>,
    /// Whether it took the focus yet: once it has been drawn.
    focused: bool,
}

/// How the person closed a dialog of the root's from inside it.
enum Closed {
    /// Cancelled, its close button, or Escape.
    Dismissed,
    /// Kept the display settings of this round.
    Kept(u64),
    /// Reverted the display settings of this round, or the time to answer ran out.
    Reverted(u64),
    /// An account was added, which closed the dialog adding it.
    Added,
    /// The host's question was answered yes (discarding unsaved changes).
    Confirmed,
}

/// A question of the host's that the root asks in its dialog's place while the host asks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostAsks {
    /// Signing in again to an account whose sign-in ended.
    Reauth,
    /// Discarding unsaved changes, for this.
    Discard(DiscardReason),
    /// Leaving the block of this id, which an editor asked to leave.
    Leave([u8; 16]),
    /// Approving the pairing request of this id.
    Pairing(u64),
}

/// Which dialog to open.
enum Asking {
    KeepDisplay(DisplayQuestion),
    /// The list of the block stack's unsent edits.
    Unsent,
    /// Adding an account: afresh, or `resumed` mid sign-in once a question that replaced it
    /// went.
    AddAccount {
        resumed: bool,
    },
    /// The question before removing the account of `key`, named `name`, kept on this device
    /// alone.
    ForgetAccount {
        key: String,
        name: String,
    },
    /// A question of the host's.
    Host(HostAsks),
    /// The organization's invitations.
    Invitations,
    /// A new invite link to the organization, as be-host's Invite member makes one, shown in
    /// the invitations' dialog.
    Invite,
    /// The organization's members.
    Members,
    /// The organization's agents.
    Agents,
    /// The person's contacts.
    Contacts,
    /// Opening a share link.
    ShareLinks,
}

/// The root's accounts while the block stack waits for the person to open or add one.
struct Onboarding {
    surface: BeuiSurface,
    /// Rung as it goes, which stops a request for a secret it made waiting.
    closing: Closing,
    /// The layer focused when they took the focus, which gets it back once they go.
    had: Option<LayerId>,
    /// Whether they took the focus yet: once drawn.
    focused: bool,
}

/// The root's lock screen while the session is locked.
struct Locked {
    surface: BeuiSurface,
    /// The lock as its document shows it.
    shown: Signal<LockState>,
    /// The host's frame time when it was shown, from which the document counts down a wait.
    shown_at: Signal<Duration>,
    /// The layer focused when it locked, which gets the focus back once unlocked.
    had: Option<LayerId>,
    /// Whether it took the focus yet: once it has been drawn.
    focused: bool,
}

/// What the root's input asked of its dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    Close,
}

impl Root {
    fn new(waker: Waker) -> Self {
        Self {
            embedding: Rc::new(RefCell::new(Embedding::new(ROOT_LAYER))),
            fonts: Rc::default(),
            beui: Rc::default(),
            own: Rc::default(),
            toasts: Rc::default(),
            menu: Rc::default(),
            menu_shows: Rc::default(),
            saving_now: Rc::new(RefCell::new(SavingState {
                generation: 0,
                unsaved: false,
                saves: 0,
                unsent: 0,
                sync: SyncState::Saved,
            })),
            shown: Rc::default(),
            saving: Rc::default(),
            keep_asking: Rc::default(),
            keep_answered: Rc::default(),
            core: Rc::new(RefCell::new(CoreView::default())),
            core_shown: Rc::default(),
            onboarding: Rc::default(),
            onboarding_shown: Rc::default(),
            onboarding_secret: Rc::default(),
            onboarding_answer: Rc::default(),
            pairing_dismissed: Rc::default(),
            wanted: Rc::default(),
            resuming: Rc::default(),
            secret: Rc::default(),
            secret_shown: Rc::default(),
            window: Rc::new(Cell::new(Window::default())),
            toast_area: Rc::default(),
            utc_offset: Rc::default(),
            dialog: Rc::default(),
            asked: Rc::default(),
            panels: Rc::default(),
            panels_made: Rc::default(),
            panel_asked: Rc::default(),
            palette: Rc::default(),
            palette_asked: Rc::default(),
            cycle_asked: Rc::default(),
            statistics_watchers: Rc::default(),
            statistics_fed: Rc::default(),
            memory_fed: Rc::default(),
            lock_state: Rc::default(),
            lock: Rc::default(),
            session: Rc::default(),
            session_rounds: Rc::default(),
            session_reload: Rc::default(),
            followed_focus: Rc::default(),
            session_notice: Rc::default(),
            session_notice_shown: Rc::default(),
            waker,
        }
    }

    /// Lays its surfaces out in the host's window as `window` says it is now: each that fills
    /// it, resized at its pixels per point when it changed.
    fn resize(&self, window: Window) {
        if self.window.replace(window) == window {
            return;
        }
        let (whole, raster) = (window.whole(), window.pixels_per_point);
        if let Some(own) = &mut *self.own.borrow_mut() {
            own.resize(whole);
            // A host that lost its device stops the root at its next frame.
            let _ = own.fill(ROOT_COLOR);
        }
        let surfaces = [&self.toasts, &self.menu, &self.session_notice];
        for surface in surfaces {
            if let Some(surface) = &mut *surface.borrow_mut() {
                surface.resize(whole, raster);
            }
        }
        if let Some(toasts) = &mut *self.toasts.borrow_mut()
            && let Some(area) = self.toast_area.get()
        {
            toasts.enter(|| area.set(window.rect()));
        }
        if let Some(dialog) = &mut *self.dialog.borrow_mut() {
            dialog.surface.resize(whole, raster);
        }
        if let Some(accounts) = &mut *self.onboarding.borrow_mut() {
            accounts.surface.resize(whole, raster);
        }
        if let Some(locked) = &mut *self.lock.borrow_mut() {
            locked.surface.resize(whole, raster);
        }
        self.palette_resize(whole, raster);
    }

    /// Hands beui the host's clocks for the frame, and makes its surfaces the first time: its
    /// own, filled with `ROOT_COLOR`, and its toasts, which beui draws in the host's fonts.
    async fn own(&self, time: Duration, wall_clock: Duration) -> Result<(), String> {
        if let Some(beui) = &*self.beui.borrow() {
            beui.set_clock(time, wall_clock);
            return Ok(());
        }
        let window = self.window.get();
        let own = OwnSurface::new(ROOT_SURFACE, window.whole());
        own.fill(ROOT_COLOR)?;
        let fonts = host::fonts()
            .await
            .map_err(|error| format!("the host gave no fonts: {error:?}"))?;
        let beui = BeuiRoot::new(&fonts, time, wall_clock)?;
        // A host that cannot answer it delivers no paste of its own, so its documents ask.
        beui.set_paste_shortcut(host::paste_shortcut().await.unwrap_or(false));
        let fonts: Fonts = be_wasm_ops::decode(&fonts).ok_or("the host's fonts cannot be read")?;
        *self.fonts.borrow_mut() = Some(Rc::new(fonts));
        let waker = self.waker.clone();
        let shown = Rc::clone(&self.shown);
        let saved = Rc::clone(&self.saving);
        let toast_area = Rc::clone(&self.toast_area);
        let toasts = BeuiSurface::new(
            &beui,
            OwnSurface::new(TOAST_SURFACE, window.whole()),
            window.pixels_per_point,
            move || waker.wake(),
            move || {
                let area = create_signal(window.rect());
                toast_area.set(Some(area));
                let toasts = create_signal(Vec::<Toast>::new());
                shown.set(Some(toasts));
                let saving = create_signal(SavingState {
                    generation: 0,
                    unsaved: false,
                    saves: 0,
                    unsent: 0,
                    sync: SyncState::Saved,
                });
                saved.set(Some(saving));
                view! {
                    <Root clear_color=ColorRole::None>
                        <Notices toasts={toasts} saving={saving} area={area} />
                    </Root>
                }
            },
        )?;
        let waker = self.waker.clone();
        let parts = self.menu_parts(MENU_SURFACE);
        let menu = BeuiSurface::new(
            &beui,
            OwnSurface::new(MENU_SURFACE, window.whole()),
            window.pixels_per_point,
            move || waker.wake(),
            move || {
                view! {
                    <Root clear_color=ColorRole::None>{parts.view(false)}</Root>
                }
            },
        )?;
        *self.menu.borrow_mut() = Some(menu);
        let waker = self.waker.clone();
        let notice_shown = Rc::clone(&self.session_notice_shown);
        let reload = Rc::clone(&self.session_reload);
        let notice = BeuiSurface::new(
            &beui,
            OwnSurface::new(SESSION_NOTICE_SURFACE, window.whole()),
            window.pixels_per_point,
            move || waker.wake(),
            move || {
                let notice = create_signal(None::<SessionShown>);
                notice_shown.set(Some(notice));
                view! {
                    <Root clear_color=ColorRole::None>
                        <SessionNotice notice={notice} reload={reload} />
                    </Root>
                }
            },
        )?;
        *self.session_notice.borrow_mut() = Some(notice);
        // F2 opens the dialog, wherever the focus is.
        self.embedding.borrow_mut().intercept(chords(false));
        *self.own.borrow_mut() = Some(own);
        *self.beui.borrow_mut() = Some(beui);
        *self.toasts.borrow_mut() = Some(toasts);
        be_wasm_ops::guest::spawn(self.clone().hear_notices());
        be_wasm_ops::guest::spawn(self.clone().hear_lock());
        be_wasm_ops::guest::spawn(self.clone().hear_saving());
        be_wasm_ops::guest::spawn(self.clone().hear_displays());
        be_wasm_ops::guest::spawn(self.clone().hear_core());
        Ok(())
    }

    /// Shows its host's notices as toasts as each change of them comes, until the host stops
    /// answering.
    async fn hear_notices(self) {
        let mut seen = 0;
        while let Ok(notices) = host::notices(seen).await {
            seen = notices.generation;
            let toasts: Vec<Toast> = notices.shown.into_iter().map(toast).collect();
            if let (Some(surface), Some(shown)) = (&mut *self.toasts.borrow_mut(), self.shown.get())
            {
                surface.enter(|| shown.set(toasts));
            }
        }
    }

    /// Shows whether its host's changes are saved as each change of it comes, until the host
    /// stops answering: the slow save's toast, in the toasts' document.
    async fn hear_saving(self) {
        let mut seen = 0;
        while let Ok(saving) = host::saving(seen).await {
            seen = saving.generation;
            *self.saving_now.borrow_mut() = saving.clone();
            self.menus_show(None, Some(&saving));
            if let (Some(surface), Some(shown)) =
                (&mut *self.toasts.borrow_mut(), self.saving.get())
            {
                surface.enter(|| shown.set(saving));
            }
        }
    }

    /// Keeps whether its host asks to keep display settings as each change of it comes, for
    /// the next frame to show, until the host stops answering.
    async fn hear_displays(self) {
        let mut seen = 0;
        while let Ok(displays) = host::displays(seen).await {
            seen = displays.generation;
            *self.keep_asking.borrow_mut() = displays.asking;
            self.waker.wake();
        }
    }

    /// Keeps what the block stack shows the person as each change of it comes, until the host
    /// stops answering, and shows it in its accounts and its dialog while they show it.
    async fn hear_core(self) {
        let mut seen = 0;
        while let Ok(view) = host::core(seen).await {
            seen = view.generation;
            *self.core.borrow_mut() = view.clone();
            self.menus_show(Some(&view), None);
            if let (Some(accounts), Some(shown)) = (
                &mut *self.onboarding.borrow_mut(),
                self.onboarding_shown.get(),
            ) {
                let view = view.clone();
                accounts.surface.enter(|| shown.set(view));
            }
            if let (Some(dialog), Some(shown)) =
                (&mut *self.dialog.borrow_mut(), self.core_shown.get())
            {
                dialog.surface.enter(|| shown.set(view));
            }
            // Whether the accounts show is the next frame's.
            self.waker.wake();
        }
    }

    /// Keeps the session's screen lock as each change of it comes, for the next frame to show,
    /// until the host stops answering.
    async fn hear_lock(self) {
        let mut seen = 0;
        while let Ok(lock) = host::lock(seen).await {
            seen = lock.generation;
            *self.lock_state.borrow_mut() = Some(lock);
            self.waker.wake();
        }
    }

    /// Shows the accounts while the block stack waits for the person to open or add one, made
    /// when it starts waiting and gone once it stops, giving the focus back to the layer that
    /// had it.
    fn settle_onboarding(&self) -> Result<(), String> {
        // A secret the host answered with goes to the onboarding that asked, or is dropped.
        if let Some(answer) = self.onboarding_answer.borrow_mut().take()
            && let Some(onboarding) = &mut *self.onboarding.borrow_mut()
            && let Some((shown, pending)) = self.onboarding_secret.get()
        {
            let held = answer.map(|secret| Held(Rc::new(secret)));
            onboarding.surface.enter(|| {
                shown.set(held);
                pending.set(false);
            });
        }
        let choosing = self.core.borrow().waits_for.is_some();
        if choosing == self.onboarding.borrow().is_some() {
            return Ok(());
        }
        if !choosing {
            self.onboarding_shown.set(None);
            self.onboarding_secret.set(None);
            let gone = self.onboarding.borrow_mut().take();
            if let Some(gone) = gone {
                gone.closing.close();
                let told = self.embedding.borrow_mut().focus(None, gone.had);
                self.heard(&told);
            }
            return Ok(());
        }
        let Some(beui) = self.beui.borrow().clone() else {
            return Ok(());
        };
        let core = self.core.borrow().clone();
        let shown = Rc::clone(&self.onboarding_shown);
        let wanted = Rc::clone(&self.wanted);
        let waker = self.waker.clone();
        let wakes = self.waker.clone();
        let asks_wake = self.waker.clone();
        let secret_shown = Rc::clone(&self.onboarding_secret);
        let answered = Rc::clone(&self.onboarding_answer);
        let closing = Closing::default();
        let closes = closing.clone();
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(ONBOARDING_SURFACE, self.window.get().whole()),
            self.window.get().pixels_per_point,
            move || waker.wake(),
            move || {
                let core = create_signal(core);
                shown.set(Some(core));
                let want = Callback::new(move |asking: Asking| {
                    *wanted.borrow_mut() = Some(asking);
                    wakes.wake();
                });
                let (secret, pending, ask) =
                    secret_asker(&secret_shown, &answered, &asks_wake, &closes);
                view! {
                    <Root>
                        <Onboarding
                            core={core}
                            want={want}
                            secret={secret}
                            pending={pending}
                            ask={ask}
                        />
                    </Root>
                }
            },
        )?;
        self.shut_palette();
        *self.onboarding.borrow_mut() = Some(Onboarding {
            surface,
            closing,
            had: None,
            focused: false,
        });
        Ok(())
    }

    /// Shows the lock as the host said last: the lock screen while the session is locked, made
    /// when it locks and gone once it is unlocked, giving the focus back to the layer that had
    /// it.
    fn settle_lock(&self) -> Result<(), String> {
        let Some(state) = self.lock_state.borrow_mut().take() else {
            return Ok(());
        };
        if !state.locked {
            let unlocked = self.lock.borrow_mut().take();
            if let Some(locked) = unlocked {
                let told = self.embedding.borrow_mut().focus(None, locked.had);
                self.heard(&told);
            }
            return Ok(());
        }
        let time = self
            .beui
            .borrow()
            .as_ref()
            .map_or(Duration::ZERO, BeuiRoot::time);
        if let Some(locked) = &mut *self.lock.borrow_mut() {
            let (shown, shown_at) = (locked.shown, locked.shown_at);
            locked.surface.enter(|| {
                shown_at.set(time);
                shown.set(state);
            });
            return Ok(());
        }
        let Some(beui) = self.beui.borrow().clone() else {
            return Ok(());
        };
        let shown = Rc::new(Cell::new(None));
        let made = Rc::clone(&shown);
        let waker = self.waker.clone();
        let utc_offset = self.utc_offset.get();
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(LOCK_SURFACE, self.window.get().whole()),
            self.window.get().pixels_per_point,
            move || waker.wake(),
            move || {
                let state = create_signal(state);
                let shown_at = create_signal(time);
                made.set(Some((state, shown_at)));
                view! {
                    <Root>
                        <LockCard state={state} shown_at={shown_at} utc_offset={utc_offset} />
                    </Root>
                }
            },
        )?;
        let (shown, shown_at) = shown
            .get()
            .ok_or("the lock screen's document made its state")?;
        let focused = self.embedding.borrow().focused(None).map(|layer| layer.id);
        let had = self.shut_palette().unwrap_or(focused);
        *self.lock.borrow_mut() = Some(Locked {
            surface,
            shown,
            shown_at,
            had,
            focused: false,
        });
        Ok(())
    }

    /// Moves the keyboard to the next app's view, or the previous while `back`, as F6 and
    /// Shift+F6 do under be-host. Not while the root's palette, dialog or onboarding is open:
    /// each keeps the keyboard until it closes, as a focus-trapping layer keeps it.
    fn cycle_focus(&self, back: bool) {
        if self.palette.borrow().is_some()
            || self.dialog.borrow().is_some()
            || self.onboarding.borrow().is_some()
        {
            return;
        }
        let told = self.embedding.borrow_mut().cycle_focus(None, back);
        self.heard(&told);
    }

    /// Hands its beui documents what landed on their surfaces. What landed on its own layer, the
    /// compositor, closes its dialog with the next frame: Escape, which it intercepts while the
    /// dialog is open and the focus is outside it. A press beside the dialog is the dialog's
    /// own, on beui's scrim.
    fn heard(&self, own: &[Delivery]) {
        let open = self.dialog.borrow().is_some();
        for delivery in own.iter().filter(|delivery| delivery.target == ROOT_LAYER) {
            let asked = match &delivery.input {
                Input::Key(stroke) if stroke.pressed && !stroke.repeat => match stroke.key {
                    Some(Key::Escape) if open => Some(Asked::Close),
                    Some(key)
                        if palette_chord().matches(KeyPress {
                            key,
                            pressed: true,
                            repeat: false,
                            modifiers: delivery.modifiers,
                        }) =>
                    {
                        self.palette_asked.set(true);
                        None
                    }
                    Some(Key::F6) => {
                        self.cycle_asked.set(Some(delivery.modifiers.shift));
                        None
                    }
                    _ => None,
                },
                _ => None,
            };
            if asked.is_some() {
                self.asked.set(asked);
            }
        }
        if let Some(toasts) = &mut *self.toasts.borrow_mut() {
            toasts.input(own);
        }
        if let Some(menu) = &mut *self.menu.borrow_mut() {
            menu.input(own);
        }
        if let Some(notice) = &mut *self.session_notice.borrow_mut() {
            notice.input(own);
        }
        if let Some(dialog) = &mut *self.dialog.borrow_mut() {
            dialog.surface.input(own);
        }
        self.panels_heard(own);
        self.palette_heard(own);
        if let Some(locked) = &mut *self.lock.borrow_mut() {
            locked.surface.input(own);
        }
        if let Some(accounts) = &mut *self.onboarding.borrow_mut() {
            accounts.surface.input(own);
        }
    }

    /// Opens or closes its dialog as its input asked, as the person closed it from inside, or
    /// as its host asks to keep display settings. It opens with its scrim, interception of
    /// Escape and, once drawn, the focus; it closes giving the focus back to the layer that had
    /// it, and Escape to whatever has the focus.
    fn settle_dialog(&self) -> Result<(), String> {
        let mut closed = self
            .dialog
            .borrow()
            .as_ref()
            .and_then(|dialog| dialog.closed.borrow_mut().take());
        let mut asked = self.asked.take();
        // A dialog waiting for a secret that cannot be had again neither closes nor gives way
        // until it comes: Escape, a press on beui's scrim and the host's questions wait.
        let holding = self
            .dialog
            .borrow()
            .as_ref()
            .is_some_and(|dialog| dialog.holds_while_pending)
            && self
                .secret_shown
                .get()
                .is_some_and(|(_, pending)| pending.get_untracked());
        if holding && asked == Some(Asked::Close) {
            asked = None;
        }
        if holding && matches!(closed, Some(Closed::Dismissed)) {
            closed = None;
        }
        let open_keep = self.dialog.borrow().as_ref().and_then(|dialog| dialog.keep);
        let signs_in = self
            .dialog
            .borrow()
            .as_ref()
            .is_some_and(|dialog| dialog.signs_in);
        let wanted = self.wanted.borrow_mut().take();
        let open_host = self
            .dialog
            .borrow()
            .as_ref()
            .and_then(|dialog| dialog.host_asks);
        let (reauthenticating, signing_in, discarding, leaving, pairing) = {
            let core = self.core.borrow();
            let dismissed = self.pairing_dismissed.borrow();
            (
                core.reauthenticating.is_some(),
                core.signing_in.is_some(),
                core.discarding.clone(),
                core.leaving.as_ref().map(|leaving| leaving.block),
                core.pairing
                    .iter()
                    .map(|request| request.id)
                    .find(|id| !dismissed.contains(id)),
            )
        };
        // The host's question, the discard first, then leaving: each answers what the person
        // just did.
        let host_asks = discarding
            .map(HostAsks::Discard)
            .or(leaving.map(HostAsks::Leave))
            .or(pairing.map(HostAsks::Pairing))
            .or(reauthenticating.then_some(HostAsks::Reauth));
        // A secret the host answered with goes to the invitations that asked, or is dropped.
        if let Some(secret) = self.secret.borrow_mut().take()
            && let Some(dialog) = &mut *self.dialog.borrow_mut()
            && dialog.receives_secret
            && let Some((shown, pending)) = self.secret_shown.get()
        {
            let link = secret.map(|secret| Held(Rc::new(secret)));
            dialog.surface.enter(|| {
                shown.set(link);
                pending.set(false);
            });
        }
        match &closed {
            Some(Closed::Kept(round)) => self.answer_display(*round, true),
            Some(Closed::Reverted(round)) => self.answer_display(*round, false),
            // Dismissing the question of keeping display settings reverts them.
            Some(Closed::Dismissed) => {
                if let Some(round) = open_keep {
                    self.answer_display(round, false);
                }
            }
            Some(Closed::Confirmed) => ask_core(CoreCommand::ConfirmDiscard),
            Some(Closed::Added) | None => {}
        }
        // Escape or a press on the scrim closes it, which for the question reverts at once.
        if closed.is_none()
            && asked == Some(Asked::Close)
            && let Some(round) = open_keep
        {
            self.answer_display(round, false);
        }
        // The question the host asks now, unless the root answered it already.
        let keep = self
            .keep_asking
            .borrow()
            .clone()
            .filter(|asking| self.keep_answered.get() != Some(asking.round));
        let keep_round = keep.as_ref().map(|asking| asking.round);
        // The host no longer asks the round shown (answered, or reverted on its own): it goes.
        let withdrawn = (open_keep.is_some() && open_keep != keep_round)
            || (open_host.is_some() && open_host != host_asks);
        // No dialog opens while the session is locked, so the focus the lock gives back is the
        // one it took; a question the host asks meanwhile shows once it is unlocked.
        let locked = self.lock.borrow().is_some()
            || self
                .lock_state
                .borrow()
                .as_ref()
                .is_some_and(|state| state.locked);
        // A question not shown yet replaces whatever dialog is open, which closes unanswered.
        let opening = match keep {
            Some(asking) if !locked && open_keep != keep_round => Some(Asking::KeepDisplay(asking)),
            // Signing in again goes on as adding an account does, showing its sign-in.
            _ if !locked
                && reauthenticating
                && signing_in
                && (open_host == Some(HostAsks::Reauth) || self.dialog.borrow().is_none()) =>
            {
                Some(Asking::AddAccount { resumed: true })
            }
            _ if !locked
                && self.dialog.borrow().is_none()
                && let Some(asks) = host_asks
                && !(asks == HostAsks::Reauth && signing_in) =>
            {
                Some(Asking::Host(asks))
            }
            _ if !locked && self.dialog.borrow().is_none() && self.resuming.take() => self
                .core
                .borrow()
                .signing_in
                .is_some()
                .then_some(Asking::AddAccount { resumed: true }),
            _ if !locked && self.dialog.borrow().is_none() => wanted,
            _ => None,
        };
        let opening = opening.filter(|_| !holding);
        let withdrawn = withdrawn && !holding;
        let closing =
            closed.is_some() || asked == Some(Asked::Close) || withdrawn || opening.is_some();
        // The host's question replacing the dialog adding an account mid sign-in leaves the
        // sign-in going, and the dialog reopens once the question goes. Closing it otherwise,
        // other than for the account added, stops adding.
        let replaced = closed.is_none()
            && asked != Some(Asked::Close)
            && matches!(opening, Some(Asking::KeepDisplay(_)));
        // Closing the host's question other than by answering yes, unless the display question
        // replaced it or the host withdrew it, answers no: keeping the changes, or leaving the
        // account for the accounts, as be-host's own dialogs do.
        if closing && !replaced && !withdrawn && !matches!(closed, Some(Closed::Confirmed)) {
            match open_host {
                Some(HostAsks::Discard(_)) => ask_core(CoreCommand::CancelDiscard),
                Some(HostAsks::Leave(_)) => ask_core(CoreCommand::CancelLeave),
                // Not now: the root asks about that request no more.
                Some(HostAsks::Pairing(id)) => self.pairing_dismissed.borrow_mut().push(id),
                Some(HostAsks::Reauth) if !matches!(opening, Some(Asking::AddAccount { .. })) => {
                    ask_core(CoreCommand::SwitchAccount)
                }
                _ => {}
            }
        }
        if closing && signs_in {
            match replaced && self.core.borrow().signing_in.is_some() {
                true => self.resuming.set(true),
                false if !matches!(closed, Some(Closed::Added)) => {
                    ask_core(CoreCommand::CancelSignIn);
                }
                false => {}
            }
        }
        if closing {
            self.close_dialog();
        }
        match opening {
            Some(opening) => self.open_dialog(opening),
            None => Ok(()),
        }
    }

    /// Tells its host whether the person kept the display settings of `round`, which it asks
    /// no more.
    fn answer_display(&self, round: u64, keep: bool) {
        self.keep_answered.set(Some(round));
        be_wasm_ops::guest::spawn(async move {
            // A host that cannot hear it has stopped, which the root hears from its frames.
            let _ = host::answer_display(round, keep).await;
        });
    }

    /// Closes its dialog, if one is open, giving the focus back to the layer that had it, and
    /// Escape to whatever has the focus.
    fn close_dialog(&self) {
        let Some(dialog) = self.dialog.borrow_mut().take() else {
            return;
        };
        dialog.closing.close();
        self.core_shown.set(None);
        self.secret_shown.set(None);
        let mut embedding = self.embedding.borrow_mut();
        embedding.intercept(chords(false));
        let told = embedding.focus(None, dialog.had);
        drop(embedding);
        self.heard(&told);
    }

    /// Opens `opening` with its scrim, interception of Escape and, once drawn, the focus.
    fn open_dialog(&self, opening: Asking) -> Result<(), String> {
        let Some(beui) = self.beui.borrow().clone() else {
            return Ok(());
        };
        let closed = Rc::<RefCell<Option<Closed>>>::default();
        let closing = Rc::clone(&closed);
        // The host's frame time it opens at, from which a question counts down to its host's.
        let shown_at = beui.time();
        let waker = self.waker.clone();
        let keep = match &opening {
            Asking::KeepDisplay(asked) => Some(asked.round),
            Asking::Unsent
            | Asking::AddAccount { .. }
            | Asking::ForgetAccount { .. }
            | Asking::Host(_)
            | Asking::Invitations
            | Asking::Invite
            | Asking::Members
            | Asking::Agents
            | Asking::Contacts
            | Asking::ShareLinks => None,
        };
        let host_asks = match &opening {
            Asking::Host(asks) => Some(*asks),
            _ => None,
        };
        let receives_secret = matches!(
            opening,
            Asking::Invitations | Asking::Invite | Asking::Agents | Asking::Contacts
        );
        let holds_while_pending = matches!(opening, Asking::Agents | Asking::Contacts);
        match opening {
            Asking::Invitations | Asking::Invite => ask_core(CoreCommand::ListInvitations),
            Asking::Members => ask_core(CoreCommand::ListMembers),
            Asking::Agents => ask_core(CoreCommand::ListAgents),
            // Each starts afresh: what an earlier opening left offered, done or failed goes.
            Asking::Contacts => ask_core(CoreCommand::ForgetContactLink),
            Asking::ShareLinks => ask_core(CoreCommand::ForgetShareLink),
            _ => {}
        }
        let secret_shown = Rc::clone(&self.secret_shown);
        let rings = Closing::default();
        let closes = rings.clone();
        let answered = Rc::clone(&self.secret);
        let wakes = self.waker.clone();
        let signs_in = matches!(opening, Asking::AddAccount { .. });
        // Adding an account starts afresh, unless resumed: whatever was being added, and why it
        // failed, goes.
        if matches!(opening, Asking::AddAccount { resumed: false }) {
            ask_core(CoreCommand::CancelSignIn);
        }
        let core = self.core.borrow().clone();
        let core_shown = Rc::clone(&self.core_shown);
        let presented = Presented::of(&opening);
        let dismissing = Rc::clone(&closed);
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(DIALOG_SURFACE, self.window.get().whole()),
            self.window.get().pixels_per_point,
            move || waker.wake(),
            move || {
                let done = Callback::new(move |how: Closed| {
                    *closing.borrow_mut() = Some(how);
                });
                // Dismissed by Escape or a press on the scrim: the question of keeping display
                // settings reverts, as be-host's does, and anything else closes as cancelled.
                let dismissed = Callback::new(move |()| {
                    let mut closed = dismissing.borrow_mut();
                    if closed.is_none() {
                        *closed = Some(match keep {
                            Some(round) => Closed::Reverted(round),
                            None => Closed::Dismissed,
                        });
                    }
                });
                let shown = match opening {
                    Asking::KeepDisplay(asked) => {
                        view! {
                            <KeepDisplayDialog asked shown_at done={done} />
                        }
                    }
                    Asking::Unsent => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <UnsentDialog core={core} done={done} />
                        }
                    }
                    Asking::AddAccount { .. } => {
                        let known: Vec<String> =
                            core.accounts.iter().map(|row| row.key.clone()).collect();
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <AddAccountDialog core={core} known done={done} />
                        }
                    }
                    Asking::Host(HostAsks::Reauth) => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <ReauthDialog core={core} done={done} />
                        }
                    }
                    Asking::Host(HostAsks::Pairing(id)) => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <PairingDialog core={core} id={id} done={done} />
                        }
                    }
                    Asking::Host(HostAsks::Leave(_)) => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <LeaveDialog core={core} done={done} />
                        }
                    }
                    Asking::Contacts => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        let (link, pending, ask) =
                            secret_asker(&secret_shown, &answered, &wakes, &closes);
                        view! {
                            <ContactsDialog
                                core={core}
                                link={link}
                                pending={pending}
                                ask={ask}
                                done={done}
                            />
                        }
                    }
                    Asking::ShareLinks => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <ShareLinkDialog core={core} done={done} />
                        }
                    }
                    Asking::Host(HostAsks::Discard(reason)) => {
                        view! {
                            <DiscardDialog reason done={done} />
                        }
                    }
                    Asking::Invitations | Asking::Invite => {
                        let invite = matches!(opening, Asking::Invite);
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        let (link, pending, ask) =
                            secret_asker(&secret_shown, &answered, &wakes, &closes);
                        if invite {
                            ask.call(SecretCommand::InviteMember);
                        }
                        view! {
                            <InvitationsDialog
                                core={core}
                                link={link}
                                pending={pending}
                                ask={ask}
                                done={done}
                            />
                        }
                    }
                    Asking::Members => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        view! {
                            <MembersDialog core={core} done={done} />
                        }
                    }
                    Asking::Agents => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        let (made, pending, ask) =
                            secret_asker(&secret_shown, &answered, &wakes, &closes);
                        view! {
                            <AgentsDialog
                                core={core}
                                made={made}
                                pending={pending}
                                ask={ask}
                                done={done}
                            />
                        }
                    }
                    Asking::ForgetAccount { key, name } => {
                        view! {
                            <ForgetAccountDialog key name done={done} />
                        }
                    }
                };
                // be-host's own dialog: a card at its width over beui's scrim, or a sheet in a
                // narrow window, which Escape and a press on the scrim dismiss.
                view! {
                    <Root clear_color=ColorRole::None>
                        <DialogFrame
                            open=true
                            kind={presented.kind}
                            width={Size::Custom(presented.width)}
                            size={presented.size}
                            on_dismiss={move || dismissed.call(())}
                        >
                            {shown}
                        </DialogFrame>
                    </Root>
                }
            },
        )?;
        let shut = self.shut_palette();
        let mut embedding = self.embedding.borrow_mut();
        let had = shut.unwrap_or_else(|| embedding.focused(None).map(|layer| layer.id));
        // Until the dialog has the focus, Escape is the root's, wherever the focus is.
        embedding.intercept(chords(true));
        drop(embedding);
        *self.dialog.borrow_mut() = Some(Dialog {
            surface,
            closed,
            keep,
            signs_in,
            host_asks,
            receives_secret,
            closing: rings,
            holds_while_pending,
            had,
            focused: false,
        });
        Ok(())
    }
}

/// How be-host presents each of the root's dialogs: its kind, width and size.
#[derive(Clone, Copy)]
struct Presented {
    kind: DialogKind,
    width: f32,
    size: DialogSize,
}

impl Presented {
    fn of(opening: &Asking) -> Self {
        let alert = |width| Self {
            kind: DialogKind::Alert,
            width,
            size: DialogSize::Small,
        };
        let dialog = |width, size| Self {
            kind: DialogKind::Dialog,
            width,
            size,
        };
        match opening {
            Asking::KeepDisplay(_) => alert(400.0),
            Asking::Unsent => dialog(460.0, DialogSize::Large),
            Asking::AddAccount { .. } => dialog(460.0, DialogSize::Small),
            Asking::ForgetAccount { .. } => alert(DIALOG_POINTS),
            Asking::Host(HostAsks::Reauth) => alert(460.0),
            Asking::Host(HostAsks::Pairing(_)) => alert(340.0),
            Asking::Host(HostAsks::Leave(_) | HostAsks::Discard(_)) => alert(DIALOG_POINTS),
            Asking::Invitations => dialog(520.0, DialogSize::Large),
            Asking::Invite => dialog(400.0, DialogSize::Small),
            Asking::Members => dialog(480.0, DialogSize::Large),
            Asking::Agents | Asking::Contacts => dialog(460.0, DialogSize::Large),
            Asking::ShareLinks => dialog(460.0, DialogSize::Small),
        }
    }
}

/// beui's default dialog width, in points, which be-host's alerts keep.
const DIALOG_POINTS: f32 = 400.0;

/// The chords the root takes ahead of whatever has the keyboard, while the session is not
/// locked: its palette's, F6 and Shift+F6, which move the keyboard between the apps' views, and
/// Escape while `escape` (its dialog is open but the keyboard is outside it). With the keyboard
/// in the dialog, Escape is its document's, which closes it once nothing in it (a popup) took
/// the key first.
fn chords(escape: bool) -> Vec<KeyChord> {
    let mut chords = vec![
        palette_chord(),
        KeyChord::new(Key::F6, Modifiers::NONE),
        KeyChord::new(Key::F6, Modifiers::SHIFT),
    ];
    if escape {
        chords.push(KeyChord::new(Key::Escape, Modifiers::NONE));
    }
    chords
}

/// A secret a dialog shows, compared by identity alone: a secret's bytes are never compared.
#[derive(Clone)]
struct Held(Rc<CoreSecret>);

impl Held {
    /// The invitation's link it is, if it is one.
    fn link(&self) -> Option<&str> {
        match &*self.0 {
            CoreSecret::Link(link) => link.as_str(),
            _ => None,
        }
    }

    /// The agent's credentials it is, if they are.
    fn agent(&self) -> Option<&be_wasm_root_api::AgentCredentials> {
        match &*self.0 {
            CoreSecret::Agent(agent) => Some(agent),
            _ => None,
        }
    }

    /// The recovery phrase it is, if it is one.
    fn phrase(&self) -> Option<&str> {
        match &*self.0 {
            CoreSecret::Phrase(phrase) => phrase.as_str(),
            _ => None,
        }
    }

    /// The pairing code it is, if it is one.
    fn code(&self) -> Option<&str> {
        match &*self.0 {
            CoreSecret::Code(code) => code.as_str(),
            _ => None,
        }
    }
}

/// What a document asking its host for secrets needs, made in it: the secret shown, whether a
/// request waits, and the request itself, answered with the frame after the host answers
/// (`settle_dialog` hands the answer over); while one waits the document asks for no other.
///
/// A request the dialog's `closing` rings for stops waiting, which cancels it: the host never
/// makes a secret it had not taken yet.
fn secret_asker(
    shown: &Rc<Cell<Option<(Signal<Option<Held>>, Signal<bool>)>>>,
    answered: &Rc<RefCell<Option<Option<CoreSecret>>>>,
    wakes: &Waker,
    closing: &Closing,
) -> (Signal<Option<Held>>, Signal<bool>, Callback<SecretCommand>) {
    let closing = closing.clone();
    let secret = create_signal(None::<Held>);
    let pending = create_signal(false);
    shown.set(Some((secret, pending)));
    let answered = Rc::clone(answered);
    let wakes = wakes.clone();
    let ask = Callback::new(move |command: SecretCommand| {
        pending.set(true);
        let answered = Rc::clone(&answered);
        let wakes = wakes.clone();
        let closing = closing.clone();
        be_wasm_ops::guest::spawn(async move {
            let mut made = pin!(host::make_secret(command));
            let mut closed = pin!(closing.closed());
            // The answer, or none once the dialog closed first: dropping the call cancels it.
            let answer = std::future::poll_fn(|context| {
                if let Poll::Ready(answer) = made.as_mut().poll(context) {
                    return Poll::Ready(Some(answer));
                }
                closed.as_mut().poll(context).map(|()| None)
            })
            .await;
            if let Some(Ok(secret)) = answer {
                *answered.borrow_mut() = Some(secret);
                wakes.wake();
            }
        });
    });
    (secret, pending, ask)
}

/// Rung once, as a dialog closes, waking whatever waits for it.
#[derive(Clone, Default)]
struct Closing(Rc<(Cell<bool>, RefCell<Option<std::task::Waker>>)>);

impl Closing {
    fn close(&self) {
        self.0.0.set(true);
        if let Some(waker) = self.0.1.borrow_mut().take() {
            waker.wake();
        }
    }

    async fn closed(&self) {
        std::future::poll_fn(|context| match self.0.0.get() {
            true => Poll::Ready(()),
            false => {
                *self.0.1.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await;
    }
}

impl PartialEq for Held {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Asks its host's block stack to do `command`.
fn ask_core(command: CoreCommand) {
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::ask_core(command).await;
    });
}

impl frames::Guest for Root {
    fn frame(&self, request: FrameRequest) -> LocalFuture<FrameDone> {
        let root = self.clone();
        Box::pin(async move {
            let time = Duration::from_nanos(request.time);
            root.resize(Window::of(&request.window));
            root.utc_offset.set(request.utc_offset);
            be_sdk_root::set_describing(request.describe);
            be_sdk_root::set_clock(
                time,
                Duration::from_nanos(request.wall_clock),
                request.utc_offset,
            );
            root.own(time, Duration::from_nanos(request.wall_clock))
                .await
                .expect("the root's host gives it fonts and a device to draw its own surface on");
            root.settle_dialog()
                .expect("the root draws its dialog with beui");
            root.settle_lock()
                .expect("the root draws its lock screen with beui");
            root.settle_onboarding()
                .expect("the root draws its accounts with beui");
            root.settle_session(time);
            let own = root
                .own
                .borrow()
                .as_ref()
                .map(|own| own.layer(ROOT_LAYER, pos2(0.0, 0.0), 1.0))
                .expect("the root made its own surface");
            if let Some(panel) = root.panel_asked.take() {
                root.show_panel(panel);
            }
            if root.palette_asked.take() {
                root.toggle_palette();
            }
            root.ask_palette();
            root.settle_palette();
            if let Some(back) = root.cycle_asked.take() {
                root.cycle_focus(back);
            }
            root.palette_frame();
            if let Some(toasts) = &mut *root.toasts.borrow_mut() {
                toasts.frame().expect("the root draws its toasts with beui");
            }
            if let Some(menu) = &mut *root.menu.borrow_mut() {
                menu.frame().expect("the root draws its menu with beui");
            }
            if let Some(notice) = &mut *root.session_notice.borrow_mut() {
                notice
                    .frame()
                    .expect("the root draws its session's notice with beui");
            }
            if let Some(dialog) = &mut *root.dialog.borrow_mut() {
                dialog
                    .surface
                    .frame()
                    .expect("the root draws its dialog with beui");
            }
            if let Some(accounts) = &mut *root.onboarding.borrow_mut() {
                accounts
                    .surface
                    .frame()
                    .expect("the root draws its accounts with beui");
            }
            if let Some(locked) = &mut *root.lock.borrow_mut() {
                locked
                    .surface
                    .frame()
                    .expect("the root draws its lock screen with beui");
            }
            // The dialog's own actions closed it: gone from this frame already.
            root.settle_dialog()
                .expect("the root draws its dialog with beui");
            // Its own surface under everything, then the session's editor filling the window with
            // the children it places, and its notice over it while it loads or after it stopped.
            let mut layers = vec![own];
            layers.extend(root.session_frame(time).await);
            layers.extend(root.session_notice_layer());
            // Its accounts while the block stack waits for one, over the app, taking every press.
            if let Some(accounts) = &*root.onboarding.borrow() {
                let mut layer = accounts.surface.layer(ONBOARDING_LAYER, pos2(0.0, 0.0));
                layer.input = LayerInput {
                    hit: Area::Whole,
                    captures: true,
                    focuses: true,
                    ..LayerInput::default()
                };
                layers.push(layer);
            }
            // Its menu while an organization's blocks are open, over the rest, hit only where
            // its button and what it opens show.
            if let Some(menu) = &*root.menu.borrow()
                && root.core.borrow().waits_for.is_none()
            {
                let mut layer = menu.layer(MENU_LAYER, pos2(0.0, 0.0));
                layer.input = LayerInput {
                    hit: menu_area(menu, root.window.get()),
                    ..LayerInput::default()
                };
                layers.push(layer);
            }
            // While the dialog is open, over the rest, filling the window: its card over beui's
            // scrim, which takes every press.
            if let Some(dialog) = &*root.dialog.borrow() {
                let mut layer = dialog.surface.layer(DIALOG_LAYER, pos2(0.0, 0.0));
                layer.input = LayerInput {
                    hit: Area::Whole,
                    captures: true,
                    focuses: true,
                    ..LayerInput::default()
                };
                layers.push(layer);
            }
            // While its palette is open, over everything else, filling the window.
            layers.extend(root.palette_layer());
            // While the session is locked, its lock screen over everything, taking every press,
            // with no toast over it: it shows nothing it would not show a stranger.
            let locked = root.lock.borrow().is_some();
            if let Some(locked) = &*root.lock.borrow() {
                let mut layer = locked.surface.layer(LOCK_LAYER, pos2(0.0, 0.0));
                layer.input = LayerInput {
                    hit: Area::Whole,
                    captures: true,
                    focuses: true,
                    ..LayerInput::default()
                };
                layers.push(layer);
            }
            // Over everything, while a toast shows: hit only where one is, and never taking the
            // focus from what the person works in.
            if let Some(toasts) = &*root.toasts.borrow()
                && !locked
                && let hit @ Area::Region(_) = toasts.overlay_area()
            {
                let mut layer = toasts.layer(TOAST_LAYER, pos2(0.0, 0.0));
                layer.input = LayerInput {
                    hit,
                    focuses: false,
                    ..LayerInput::default()
                };
                layers.push(layer);
            }
            let list = LayerList {
                windows: vec![WindowLayers {
                    window: None,
                    layers,
                }],
            };
            let drawn_layers: Vec<LayerId> = list
                .windows
                .iter()
                .flat_map(|window| window.layers.iter().map(|layer| layer.id))
                .collect();
            // The list is taken as drawn first, so an ask can focus a layer it holds; what the
            // apps want is read after, as it was before (nothing between changes it).
            let drawn = root.embedding.borrow_mut().drew(list);
            // A child asked to take the keyboard that now shows takes it there, unless the root's
            // own takes it below.
            let given = root.keyboard_into_children(&drawn_layers);
            let mut embedding = root.embedding.borrow_mut();
            let wants = embedding.wants();
            // The lock screen, or else the dialog, or else the accounts, drawn for the first time
            // takes the focus.
            let focus = match (
                &mut *root.lock.borrow_mut(),
                &mut *root.dialog.borrow_mut(),
                &mut *root.onboarding.borrow_mut(),
            ) {
                (Some(locked), _, _) if !locked.focused => {
                    locked.focused = true;
                    embedding.focus(None, Some(LOCK_LAYER))
                }
                (None, Some(dialog), _) if !dialog.focused => {
                    dialog.focused = true;
                    embedding.focus(None, Some(DIALOG_LAYER))
                }
                (None, None, Some(accounts)) if !accounts.focused => {
                    // What had the focus as they take it, which gets it back once they go.
                    accounts.had = embedding.focused(None).map(|layer| layer.id);
                    accounts.focused = true;
                    embedding.focus(None, Some(ONBOARDING_LAYER))
                }
                _ => root.palette_focus(&mut embedding),
            };
            // While locked, the root takes no key ahead of the lock screen. Otherwise Escape is
            // the root's while its dialog is open and the focus is outside it.
            let outside = root.dialog.borrow().is_some()
                && embedding.focused(None).map(|layer| layer.id) != Some(DIALOG_LAYER);
            embedding.intercept(match locked {
                true => Vec::new(),
                false => chords(outside),
            });
            drop(embedding);
            root.heard(&drawn.own);
            root.heard(&given);
            root.heard(&focus);
            root.follow_focus();
            let mut embedding = root.embedding.borrow_mut();
            let requests = embedding.requests(None);
            // Its documents want a frame at once (one heard input or was woken) or later.
            let documents = [&root.toasts, &root.menu, &root.session_notice]
                .into_iter()
                .filter_map(|surface| surface.borrow().as_ref().and_then(BeuiSurface::next));
            let dialog = root
                .dialog
                .borrow()
                .as_ref()
                .and_then(|dialog| dialog.surface.next());
            let lock = root
                .lock
                .borrow()
                .as_ref()
                .and_then(|locked| locked.surface.next());
            let accounts = root
                .onboarding
                .borrow()
                .as_ref()
                .and_then(|accounts| accounts.surface.next());
            let next = wants
                .next
                .into_iter()
                .chain(root.panels_next())
                .chain(documents)
                .chain(dialog)
                .chain(root.palette_next())
                .chain(lock)
                .chain(accounts)
                .min();
            // The input method is where the focused surface edits: an app's screen, or the
            // field of its dialog.
            let ime = embedding.ime(None).or_else(|| {
                let focused = embedding.focused(None)?;
                let dialog = root.dialog.borrow();
                let lock = root.lock.borrow();
                let accounts = root.onboarding.borrow();
                match focused.id {
                    DIALOG_LAYER => dialog.as_ref()?.surface.ime(focused),
                    LOCK_LAYER => lock.as_ref()?.surface.ime(focused),
                    ONBOARDING_LAYER => accounts.as_ref()?.surface.ime(focused),
                    PALETTE_LAYER => root.palette_ime(focused),
                    _ => None,
                }
            });
            // Its accessibility tree while its host asks for it: its own surfaces' and its
            // apps' screens', in the order its layers are drawn.
            let accessibility = request.describe.then(|| {
                let window = root.window.get();
                let size = be_lib_geometry::vec2(window.width, window.height);
                be_wasm_ops::encode(&embedding.describe(None, size, root.described()))
            });
            FrameDone {
                accessibility,
                layers: drawn.layers,
                next: next.map(|at| u64::try_from(at.as_nanos()).unwrap_or(u64::MAX)),
                owed: wants.owed || root.session_loading(),
                ime: ime.map(|area| area.encode()),
                every_sample: requests.every_sample,
                cursor: requests.cursor.name().to_owned(),
                pointer_locked: requests.pointer_locked,
            }
        })
    }
}

impl input::Guest for Root {
    fn deliver(&self, window: Option<u64>, events: Vec<u8>) -> LocalFuture<()> {
        let routed = self
            .embedding
            .borrow_mut()
            .input(window, &events)
            .unwrap_or_default();
        self.heard(&routed.own);
        self.revoke_children(&routed.pressed);
        self.follow_focus();
        // What it asked of the dialog is done with the next frame, which this asks for.
        if self.asked.get().is_some() {
            self.waker.wake();
        }
        Box::pin(async {})
    }
}

impl Root {
    /// What each surface of its own describes of itself (`BeuiSurface::describe`), by surface:
    /// those its layers do not show are left out of its tree (`Embedding::describe`).
    fn described(&self) -> Vec<(u32, be_sdk_api::accesskit::TreeUpdate)> {
        let described = |surface: &BeuiSurface| Some((surface.id(), surface.describe()?));
        let mut trees: Vec<(u32, be_sdk_api::accesskit::TreeUpdate)> = [
            &self.toasts,
            &self.menu,
            &self.session_notice,
        ]
        .into_iter()
        .filter_map(|surface| surface.borrow().as_ref().and_then(described))
        .collect();
        trees.extend(self.dialog.borrow().as_ref().and_then(|dialog| described(&dialog.surface)));
        trees.extend(
            self.onboarding
                .borrow()
                .as_ref()
                .and_then(|accounts| described(&accounts.surface)),
        );
        trees.extend(self.lock.borrow().as_ref().and_then(|locked| described(&locked.surface)));
        trees.extend(self.palette_described());
        trees.extend(self.panels_described());
        trees
    }
}

impl paintings::Guest for Root {
    /// Its own surface's painting, all `ROOT_COLOR`, and what beui painted of each surface it
    /// shows now: its toasts while one shows, its dialog and scrim, its onboarding, the host
    /// panels it draws and its lock screen.
    fn paintings(&self) -> LocalFuture<Vec<u8>> {
        let own = self.own.borrow().as_ref().map(|own| {
            let (width, height) = own.size();
            let painting = Painting {
                frame: PaintFrame {
                    size: [width, height],
                    pixels_per_point: 1.0,
                    background: ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8),
                    primitives: Vec::new(),
                },
                textures: Default::default(),
            };
            (u64::from(own.id()), painting)
        });
        let painted = |surface: &BeuiSurface| Some((u64::from(surface.id()), surface.painting()?));
        // Its toasts only while one shows, as its layer list shows them.
        let toasts = self
            .toasts
            .borrow()
            .as_ref()
            .filter(|toasts| toasts.overlay_area() != Area::Nowhere)
            .and_then(painted);
        // Its dialog only while it is open, as its layer list shows it.
        let dialog = self
            .dialog
            .borrow()
            .as_ref()
            .and_then(|dialog| painted(&dialog.surface));
        // Its accounts while they show, and its lock screen while locked.
        let accounts = self
            .onboarding
            .borrow()
            .as_ref()
            .and_then(|accounts| painted(&accounts.surface));
        let lock = self
            .lock
            .borrow()
            .as_ref()
            .and_then(|locked| painted(&locked.surface));
        // Its menu while nothing waits, as its layer list shows it.
        let menu = match self.core.borrow().waits_for {
            Some(_) => None,
            None => self.menu.borrow().as_ref().and_then(painted),
        };
        // The session's notice while it shows one, as its layer list shows it.
        let notice = match self.session_notice_layer() {
            Some(_) => self.session_notice.borrow().as_ref().and_then(painted),
            None => None,
        };
        let session = self.session_screens();
        let panels = self.panels_painted();
        let mut screens: Vec<_> = own
            .into_iter()
            .chain(panels)
            .chain(notice)
            .chain(menu)
            .chain(accounts)
            .chain(dialog)
            .chain(self.palette_painted())
            .chain(toasts)
            .chain(lock)
            .collect();
        Box::pin(async move {
            // The session's editor's screen and those of the children it places, as their apps
            // paint them, each by the id of the layer that shows it. An app answers at once, and
            // one that stopped answers nothing, so this waits for no frame: a deadline would
            // leave a screen out of a slow frame's painting and in the next's.
            for (app, screen, layer) in session {
                if let Some(painted) = app.paintings().await
                    && let Ok(painted) = ScreenPaintings::decode(&painted)
                    && let Some((_, painting)) = painted
                        .screens
                        .into_iter()
                        .find(|(painted, _)| *painted == screen.0)
                {
                    screens.push((layer.0, painting));
                }
            }
            ScreenPaintings {
                screens,
                overflows: Vec::new(),
            }
            .encode()
        })
    }
}

pub struct Embedder {
    executor: Executor,
}

impl Guest for Embedder {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            executor: be_wasm_root_api::guest::executor(Root::new(waker)),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        // What the gpu settled since the last step, as be-sdk polls for an app.
        be_wasm_gpu_guest::poll();
        self.executor.step(inbound, outbound)?;
        Ok(Flow::Running)
    }
}

be_wasm_guest_abi::guest!(Embedder);
