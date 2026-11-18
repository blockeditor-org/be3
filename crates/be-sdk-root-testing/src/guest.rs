//! The root itself, built for the apps' platform alone.

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::{Pin, pin},
    rc::Rc,
    task::Poll,
    time::Duration,
};

use be_lib_geometry::pos2;
use be_lib_layers::{Area, LayerId, LayerInput, LayerList, Surface, WindowLayers};
use be_lib_ui_types::input::{Input, Key, KeyChord, Modifiers, TouchPhase};
use be_sdk_api::{
    EditorInstanceId, Fonts, OpenBlock, Opening, OpeningKind, ParentView, Pos2 as ViewPos2,
    ScreenRequest, SurfaceFormat, SurfaceSpec, Vec2 as ViewVec2, ViewId, ViewPlacement,
    ViewportMetrics,
};
use be_sdk_beui_root::{BeuiRoot, BeuiSurface};
use be_sdk_root::{Delivery, Embedded, Embedding, OwnSurface, Spawn};
use be_wasm_guest_abi::{Flow, Guest, Waker};
use be_wasm_ops::guest::{Executor, LocalFuture};
use be_wasm_password::Password;
use be_wasm_root_api::{
    AccountRow, AgentRow, MemoryMinutes, PairingApproval, ContactCard, ContactMismatch, ContactRow, ShareAccess, SharePreview,
    CoreCommand, CoreSecret, MemberRole, MemberRow, RoleChange, SecretCommand, CoreView, CoreWait, DiscardReason, InvitationRow,
    InvitationState, OrganizationRow, OrganizationsListed, ProfileRow,
    SessionRow, DisplayQuestion, FrameDone, FrameRequest, HostStatistics, LockState,
    NewAccount, Notice, PowerAction, Refusal, SavingState, ServerAccount, SetAside, StatisticRow, SyncState, UnsentEdit,
    guest::{
        exports::{frames, input, paintings},
        host,
    },
};
use beui::icons::{Glyph, ICON_LOG_OUT, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW, ICON_TRASH};
use beui::native::painting::{Frame as PaintFrame, Painting, ScreenPaintings};
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Align, Align2, Direction, Justify, KeyPhase, NodeId, Overflow, Pos2, Rect, TextAlign, vec2,
};

use crate::{
    ADD_ACCOUNT_TITLE, AGENTS_TITLE, APP, APP_ENTRY, CLOSES, CONTACTS_TITLE, DIALOG_LAYER,
    DIALOG_ORIGIN, DIALOG_SIZE, DIALOG_SURFACE, DIALOG_TITLE, DISCARD_TITLE, DISCARD_UNSENT_TITLE,
    FAILURE_TITLE, FORGET_ACCOUNT_TITLE, INVITATIONS_TITLE, KEEP_DISPLAY_TITLE, LATE_LAYER, LATER,
    LATER_TEXT, LEAVE_TITLE, LOCK_LAYER, LOCK_SURFACE, LOCKS_SESSION, MEMBERS_TITLE,
    ONBOARDING_LAYER, ONBOARDING_SURFACE, OPENS_AGENTS, OPENS_CONTACTS, OPENS_DIALOG,
    OPENS_INVITATIONS, OPENS_MEMBERS, OPENS_SHARE_LINKS, OPENS_STATISTICS, OPENS_UNSENT,
    ORGANIZATIONS_TITLE, ORIGIN, PAIRING_TITLE, RASTER, REAUTH_TITLE, RECOVERY_TITLE,
    REMOVE_CONTACT_TITLE, REMOVE_MEMBER_TITLE, RESIZED, RESIZES, ROOT_COLOR, ROOT_LAYER,
    ROOT_SURFACE, SCREEN, SCREEN_SIZE, SCRIM_COLOR, SCRIM_LAYER, SCRIM_SURFACE, SESSIONS_TITLE,
    SHARE_LINK_TITLE, SIGN_IN_TITLE, SIZE, SLOW_SAVE, SLOW_SAVE_TITLE, SLOW_SAVE_TOAST,
    STATISTICS_AREA, STATISTICS_LAYER, STATISTICS_ORIGIN, STATISTICS_SURFACE, STATISTICS_TITLE,
    STRAY_PASSWORD, TEXT, TEXT_LAYER, TEXT_STRIP, TEXT_SURFACE, TOAST_AREA, TOAST_LAYER,
    TOAST_SURFACE, UNLOCK_TITLE, UNLOCKS_TWICE, UNPRESENTED_SURFACE, UNSENT_TITLE,
};

/// The triangle's editor, by its manifest's id.
const EDITOR: [u8; 16] = [
    0x52, 0x54, 0xd0, 0xa5, 0xae, 0x87, 0x4a, 0xfe, 0x84, 0x58, 0x50, 0xfd, 0xe9, 0xcc, 0x0d, 0x2b,
];

/// The triangle's block type, by its manifest's id.
const BLOCK_TYPE: [u8; 16] = [
    0x77, 0x67, 0x70, 0x75, 0x2d, 0x74, 0x72, 0x69, 0x61, 0x6e, 0x67, 0x6c, 0x65, 0x30, 0x30, 0x31,
];

#[derive(Clone)]
struct Root {
    embedding: Rc<RefCell<Embedding>>,
    app: Rc<RefCell<Option<Embedded>>>,
    /// The fonts its host gave, which it tells the app it spawns, had on its first frame.
    fonts: Rc<RefCell<Option<Rc<Fonts>>>>,
    /// What its surfaces drawn with beui share, made on its first frame.
    beui: Rc<RefCell<Option<BeuiRoot>>>,
    /// Its own surface, filled with `ROOT_COLOR` on its first frame.
    own: Rc<RefCell<Option<OwnSurface>>>,
    /// The strip of text over it, which beui draws, made on its first frame.
    text: Rc<RefCell<Option<BeuiSurface>>>,
    /// Its host's notices as toasts, which beui draws, made on its first frame.
    toasts: Rc<RefCell<Option<BeuiSurface>>>,
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
    /// The surface a `RESIZES` frame configures and never presents.
    unpresented: Rc<RefCell<Option<OwnSurface>>>,
    /// The scrim under its dialog, filled with `SCRIM_COLOR` on its first frame.
    scrim: Rc<RefCell<Option<OwnSurface>>>,
    /// Its dialog, while it is open.
    dialog: Rc<RefCell<Option<Dialog>>>,
    /// What its input asked of the dialog, done with the next frame.
    asked: Rc<Cell<Option<Asked>>>,
    /// The name the strip shows in place of `TEXT` once the dialog renamed it, which its
    /// document sets.
    named: Rc<Cell<Option<Signal<Option<String>>>>>,
    /// Its panel of the host's statistics, while it is open.
    statistics: Rc<RefCell<Option<BeuiSurface>>>,
    /// The statistics that panel shows, which the host's answers set.
    shown_statistics: Rc<Cell<Option<Signal<HostStatistics>>>>,
    /// The memory over time that panel graphs, which the host's answers set.
    shown_memory: Rc<Cell<Option<Signal<MemoryMinutes>>>>,
    /// Counts the panel's openings, so a watch begun for an earlier one stops.
    statistics_opened: Rc<Cell<u64>>,
    /// The session's screen lock as the host said last, until a frame shows it.
    lock_state: Rc<RefCell<Option<LockState>>>,
    /// Its lock screen, while the session is locked.
    lock: Rc<RefCell<Option<Locked>>>,
    /// Wakes the root, and so its host, for a frame its beui document wants.
    waker: Waker,
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
    /// Renamed to this.
    Renamed(String),
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
    Rename,
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

/// What the root's input asked of its dialog, or of its statistics panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    Open,
    OpenUnsent,
    OpenInvitations,
    OpenMembers,
    OpenAgents,
    OpenContacts,
    OpenShareLinks,
    Close,
    ToggleStatistics,
}

impl Root {
    fn new(waker: Waker) -> Self {
        Self {
            embedding: Rc::new(RefCell::new(Embedding::new(ROOT_LAYER))),
            app: Rc::default(),
            fonts: Rc::default(),
            beui: Rc::default(),
            own: Rc::default(),
            text: Rc::default(),
            toasts: Rc::default(),
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
            unpresented: Rc::default(),
            scrim: Rc::default(),
            dialog: Rc::default(),
            asked: Rc::default(),
            named: Rc::default(),
            statistics: Rc::default(),
            shown_statistics: Rc::default(),
            shown_memory: Rc::default(),
            statistics_opened: Rc::default(),
            lock_state: Rc::default(),
            lock: Rc::default(),
            waker,
        }
    }

    /// Hands beui the host's clocks for the frame, and makes its surfaces the first time: its
    /// own, filled with `ROOT_COLOR`, and the strip of text beui draws in the host's fonts.
    async fn own(&self, time: Duration, wall_clock: Duration) -> Result<(), String> {
        if let Some(beui) = &*self.beui.borrow() {
            beui.set_clock(time, wall_clock);
            return Ok(());
        }
        let own = OwnSurface::new(ROOT_SURFACE, SIZE);
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
        let named = Rc::clone(&self.named);
        let text = BeuiSurface::new(
            &beui,
            OwnSurface::new(TEXT_SURFACE, TEXT_STRIP),
            1.0,
            move || waker.wake(),
            move || {
                let name = create_signal(None::<String>);
                named.set(Some(name));
                view! {
                    <Root clear_color=ColorRole::None>
                        <Strip name={name} />
                    </Root>
                }
            },
        )?;
        let waker = self.waker.clone();
        let shown = Rc::clone(&self.shown);
        let saved = Rc::clone(&self.saving);
        let toasts = BeuiSurface::new(
            &beui,
            OwnSurface::new(TOAST_SURFACE, TOAST_AREA),
            1.0,
            move || waker.wake(),
            move || {
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
                        <Notices toasts={toasts} saving={saving} />
                    </Root>
                }
            },
        )?;
        let scrim = OwnSurface::new(SCRIM_SURFACE, SIZE);
        scrim.fill(SCRIM_COLOR)?;
        // F2 opens the dialog, wherever the focus is.
        self.embedding.borrow_mut().intercept(chords(false));
        *self.own.borrow_mut() = Some(own);
        *self.scrim.borrow_mut() = Some(scrim);
        *self.beui.borrow_mut() = Some(beui);
        *self.text.borrow_mut() = Some(text);
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
            OwnSurface::new(ONBOARDING_SURFACE, SIZE),
            1.0,
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
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(LOCK_SURFACE, SIZE),
            1.0,
            move || waker.wake(),
            move || {
                let state = create_signal(state);
                let shown_at = create_signal(time);
                made.set(Some((state, shown_at)));
                view! {
                    <Root>
                        <LockCard state={state} shown_at={shown_at} />
                    </Root>
                }
            },
        )?;
        let (shown, shown_at) = shown
            .get()
            .ok_or("the lock screen's document made its state")?;
        let had = self.embedding.borrow().focused(None).map(|layer| layer.id);
        *self.lock.borrow_mut() = Some(Locked {
            surface,
            shown,
            shown_at,
            had,
            focused: false,
        });
        Ok(())
    }

    /// The triangle, spawned the first time.
    fn app(&self, now: Duration) -> Embedded {
        let mut app = self.app.borrow_mut();
        let app = app.get_or_insert_with(|| {
            let app = Embedded::spawn(
                Spawn {
                    app: APP.to_owned(),
                    module: APP_ENTRY.to_owned(),
                    opening: opening(),
                    screens: vec![screen()],
                    surface: SurfaceSpec {
                        format: SurfaceFormat::Rgba8Unorm,
                        render: true,
                    },
                    fonts: self.fonts.borrow().clone().unwrap_or_default(),
                },
                now,
            );
            self.embedding.borrow_mut().embed(app.clone());
            app
        });
        app.clone()
    }

    /// Hands its beui documents what landed on their surfaces: its strip of text, its toasts
    /// and its dialog. What landed on its own layer, the compositor, opens the dialog (F2,
    /// which it intercepts) or closes it (Escape, which it intercepts while the dialog is open,
    /// or a press the scrim captured), with the next frame.
    fn heard(&self, own: &[Delivery]) {
        let open = self.dialog.borrow().is_some();
        for delivery in own.iter().filter(|delivery| delivery.target == ROOT_LAYER) {
            let asked = match &delivery.input {
                Input::Key(stroke) if stroke.pressed && !stroke.repeat => match stroke.key {
                    Some(key) if key == OPENS_DIALOG && !open => Some(Asked::Open),
                    Some(key) if key == OPENS_UNSENT && !open => Some(Asked::OpenUnsent),
                    Some(key) if key == OPENS_INVITATIONS && !open => Some(Asked::OpenInvitations),
                    Some(key) if key == OPENS_MEMBERS && !open => Some(Asked::OpenMembers),
                    Some(key) if key == OPENS_AGENTS && !open => Some(Asked::OpenAgents),
                    Some(key) if key == OPENS_CONTACTS && !open => Some(Asked::OpenContacts),
                    Some(key) if key == OPENS_SHARE_LINKS && !open => Some(Asked::OpenShareLinks),
                    Some(key) if key == OPENS_STATISTICS => Some(Asked::ToggleStatistics),
                    Some(key) if key == UNLOCKS_TWICE => {
                        unlock(STRAY_PASSWORD.to_owned());
                        unlock(STRAY_PASSWORD.to_owned());
                        None
                    }
                    Some(key) if key == LOCKS_SESSION => {
                        be_wasm_ops::guest::spawn(async {
                            // A host that cannot hear it has stopped, which the root hears from
                            // its frames.
                            let _ = host::lock_session().await;
                        });
                        None
                    }
                    Some(Key::Escape) if open => Some(Asked::Close),
                    _ => None,
                },
                Input::PointerButton { pressed: true, .. } if open => Some(Asked::Close),
                Input::Touch {
                    phase: TouchPhase::Start,
                    ..
                } if open => Some(Asked::Close),
                _ => None,
            };
            match asked {
                // Toggled at once: what it draws is the next frame's.
                Some(Asked::ToggleStatistics) => self.toggle_statistics(),
                Some(asked) => self.asked.set(Some(asked)),
                None => {}
            }
        }
        if let Some(text) = &mut *self.text.borrow_mut() {
            text.input(own);
        }
        if let Some(toasts) = &mut *self.toasts.borrow_mut() {
            toasts.input(own);
        }
        if let Some(dialog) = &mut *self.dialog.borrow_mut() {
            dialog.surface.input(own);
        }
        if let Some(statistics) = &mut *self.statistics.borrow_mut() {
            statistics.input(own);
        }
        if let Some(locked) = &mut *self.lock.borrow_mut() {
            locked.surface.input(own);
        }
        if let Some(accounts) = &mut *self.onboarding.borrow_mut() {
            accounts.surface.input(own);
        }
    }

    /// Opens its panel of the host's statistics, watching them while it is open, or closes it,
    /// which stops the watch.
    fn toggle_statistics(&self) {
        let opened = self.statistics_opened.get() + 1;
        self.statistics_opened.set(opened);
        if self.statistics.borrow_mut().take().is_some() {
            be_wasm_ops::guest::spawn(async {
                // A host that cannot hear it has stopped, which the root hears from its frames.
                let _ = host::watch_statistics(false).await;
            });
            return;
        }
        let Some(beui) = self.beui.borrow().clone() else {
            return;
        };
        let waker = self.waker.clone();
        let shown = Rc::clone(&self.shown_statistics);
        let graphed = Rc::clone(&self.shown_memory);
        let panel = BeuiSurface::new(
            &beui,
            OwnSurface::new(STATISTICS_SURFACE, STATISTICS_AREA),
            1.0,
            move || waker.wake(),
            move || {
                let statistics = create_signal(HostStatistics {
                    generation: 0,
                    performance: Vec::new(),
                    memory: Vec::new(),
                    memory_file: None,
                });
                shown.set(Some(statistics));
                let history = create_signal(MemoryMinutes::default());
                graphed.set(Some(history));
                view! {
                    <Root>
                        <StatisticsPanel statistics={statistics} history={history} />
                    </Root>
                }
            },
        );
        match panel {
            Ok(panel) => *self.statistics.borrow_mut() = Some(panel),
            // The panel is for debugging: one that cannot be drawn is left closed.
            Err(_) => return,
        }
        be_wasm_ops::guest::spawn(self.clone().hear_statistics(opened));
        be_wasm_ops::guest::spawn(self.clone().hear_memory(opened));
    }

    /// Watches the host's memory over time and graphs each change of it in the panel opened as
    /// the `opened`th, until it closes or the host stops answering. Its rows change every frame,
    /// these once a minute, so they are a watch of their own.
    async fn hear_memory(self, opened: u64) {
        let mut seen = 0;
        while let Ok(history) = host::memory_history(seen).await {
            if self.statistics_opened.get() != opened {
                return;
            }
            seen = history.generation;
            if let (Some(panel), Some(shown)) =
                (&mut *self.statistics.borrow_mut(), self.shown_memory.get())
            {
                panel.enter(|| shown.set(history));
            }
        }
    }

    /// Watches the host's statistics and shows each change of them in the panel opened as the
    /// `opened`th, until it closes or the host stops answering.
    async fn hear_statistics(self, opened: u64) {
        if host::watch_statistics(true).await.is_err() {
            return;
        }
        let mut seen = 0;
        while let Ok(statistics) = host::statistics(seen).await {
            if self.statistics_opened.get() != opened {
                return;
            }
            seen = statistics.generation;
            if let (Some(panel), Some(shown)) = (
                &mut *self.statistics.borrow_mut(),
                self.shown_statistics.get(),
            ) {
                panel.enter(|| shown.set(statistics));
            }
        }
    }

    /// Opens or closes its dialog as its input asked, as the person closed it from inside, or
    /// as its host asks to keep display settings. It opens with its scrim, interception of
    /// Escape and, once drawn, the focus; it closes giving the focus back to the layer that had
    /// it, and Escape to whatever has the focus.
    fn settle_dialog(&self) -> Result<(), String> {
        let closed = self
            .dialog
            .borrow()
            .as_ref()
            .and_then(|dialog| dialog.closed.borrow_mut().take());
        let mut asked = self.asked.take();
        // A dialog waiting for a secret that cannot be had again neither closes nor gives way
        // until it comes: a press on the scrim, Escape from outside and the host's questions wait.
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
            Some(Closed::Renamed(name)) => {
                if let (Some(text), Some(shown)) = (&mut *self.text.borrow_mut(), self.named.get())
                {
                    let name = name.clone();
                    text.enter(|| shown.set(Some(name)));
                }
            }
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
            _ if !locked && asked == Some(Asked::Open) && self.dialog.borrow().is_none() => {
                Some(Asking::Rename)
            }
            _ if !locked && asked == Some(Asked::OpenUnsent) && self.dialog.borrow().is_none() => {
                Some(Asking::Unsent)
            }
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
            _ if !locked
                && asked == Some(Asked::OpenInvitations)
                && self.dialog.borrow().is_none() =>
            {
                Some(Asking::Invitations)
            }
            _ if !locked && asked == Some(Asked::OpenMembers) && self.dialog.borrow().is_none() => {
                Some(Asking::Members)
            }
            _ if !locked && asked == Some(Asked::OpenAgents) && self.dialog.borrow().is_none() => {
                Some(Asking::Agents)
            }
            _ if !locked
                && asked == Some(Asked::OpenContacts)
                && self.dialog.borrow().is_none() =>
            {
                Some(Asking::Contacts)
            }
            _ if !locked
                && asked == Some(Asked::OpenShareLinks)
                && self.dialog.borrow().is_none() =>
            {
                Some(Asking::ShareLinks)
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
            Asking::Rename
            | Asking::Unsent
            | Asking::AddAccount { .. }
            | Asking::ForgetAccount { .. }
            | Asking::Host(_)
            | Asking::Invitations
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
            Asking::Invitations | Asking::Agents | Asking::Contacts
        );
        let holds_while_pending = matches!(opening, Asking::Agents | Asking::Contacts);
        match opening {
            Asking::Invitations => ask_core(CoreCommand::ListInvitations),
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
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(DIALOG_SURFACE, DIALOG_SIZE),
            1.0,
            move || waker.wake(),
            move || {
                let done = Callback::new(move |how: Closed| {
                    *closing.borrow_mut() = Some(how);
                });
                let shown = match opening {
                    Asking::Rename => view! {
                        <RenameDialog done={done} />
                    },
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
                    Asking::Invitations => {
                        let core = create_signal(core);
                        core_shown.set(Some(core));
                        let (link, pending, ask) =
                            secret_asker(&secret_shown, &answered, &wakes, &closes);
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
                view! {
                    <Root clear_color=ColorRole::None>{shown}</Root>
                }
            },
        )?;
        let mut embedding = self.embedding.borrow_mut();
        let had = embedding.focused(None).map(|layer| layer.id);
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

/// The chords the root takes ahead of the focus, while the session is not locked: F2, which
/// opens the dialog, F3, which opens and closes the statistics panel, F4, which locks the
/// session, F5, the test fixture, F7, which opens the list of unsent edits, and Escape while
/// the dialog is open but the focus is outside it. With the focus in the dialog, Escape is its
/// document's, which closes it once nothing in it (a popup) took the key first.
fn chords(escape: bool) -> Vec<KeyChord> {
    let mut chords = vec![
        KeyChord::new(OPENS_DIALOG, Modifiers::NONE),
        KeyChord::new(OPENS_STATISTICS, Modifiers::NONE),
        KeyChord::new(LOCKS_SESSION, Modifiers::NONE),
        KeyChord::new(UNLOCKS_TWICE, Modifiers::NONE),
        KeyChord::new(OPENS_UNSENT, Modifiers::NONE),
        KeyChord::new(OPENS_INVITATIONS, Modifiers::NONE),
        KeyChord::new(OPENS_MEMBERS, Modifiers::NONE),
        KeyChord::new(OPENS_AGENTS, Modifiers::NONE),
        KeyChord::new(OPENS_CONTACTS, Modifiers::NONE),
        KeyChord::new(OPENS_SHARE_LINKS, Modifiers::NONE),
    ];
    if escape {
        chords.push(KeyChord::new(Key::Escape, Modifiers::NONE));
    }
    chords
}

/// The dialog: a name, which it focuses, and Cancel and Rename, filling its surface. `done`
/// hears how it closed: with the name, or none.
#[component]
fn RenameDialog(done: Callback<Closed>) -> NodeId {
    let name = create_signal(String::new());
    let rename = move || done.call(Closed::Renamed(name.get()));
    // Escape closes it once nothing inside it, such as a popup, took the key first.
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                done.call(Closed::Dismissed);
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                title=DIALOG_TITLE
                scrolls=false
                actions={vec![DialogAction::cancel("Cancel"), DialogAction::primary("Rename", rename)]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <TextInput
                    value={name}
                    label="Name"
                    focused=true
                    on_submit={move |_: String| rename()}
                />
            </DialogBody>
        </Frame>
    }
}

/// A notice of its host's as a toast: one that says something failed is an alert, which stays
/// until it is answered.
fn toast(notice: Notice) -> Toast {
    Toast {
        id: notice.id,
        message: notice.message,
        danger: notice.failure,
        actions: notice
            .actions
            .into_iter()
            .map(|action| ToastAction {
                key: action.key,
                label: action.label,
            })
            .collect(),
        ..Toast::default()
    }
}

/// Tells its host how the person answered its notice `id`: with the key of one of its
/// actions, or none to dismiss it.
fn answer(id: u64, action: Option<String>) {
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::answer_notice(id, action).await;
    });
}

/// Its host's notices as toasts at the top right of its surface, each gone at once from here
/// when dismissed, as its host lets go of it.
#[component]
fn Notices(toasts: Signal<Vec<Toast>>, saving: Signal<SavingState>) -> NodeId {
    let (width, height) = TOAST_AREA;
    let area = Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32));
    let slow = slow_save(saving);
    let shown = create_memo(move || {
        let mut shown = toasts.get();
        if slow.shown.get() {
            shown.insert(
                0,
                Toast {
                    id: SLOW_SAVE_TOAST,
                    title: SLOW_SAVE_TITLE.to_owned(),
                    message: "This is taking longer than usual. Keep it open until it finishes."
                        .to_owned(),
                    sticky: true,
                    ..Toast::default()
                },
            );
        }
        shown
    });
    view! {
        <Toasts
            anchor={OverlayAnchor::Rect(area)}
            edge=Edge::TopEnd
            toasts={shown}
            on_dismiss={move |id: u64| {
                if id == SLOW_SAVE_TOAST {
                    slow.dismissed.set(true);
                    return;
                }
                toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                answer(id, None);
            }}
            on_action={move |(id, key): (u64, String)| answer(id, Some(key))}
        />
    }
}

/// The slow save's toast: shown once changes have stayed unsaved for `SLOW_SAVE` by the host's
/// frame clock, a save finishing starting the wait afresh, until they are saved or the person
/// dismisses it.
#[derive(Clone, Copy)]
struct SlowSave {
    shown: Memo<bool>,
    dismissed: Signal<bool>,
}

fn slow_save(saving: Signal<SavingState>) -> SlowSave {
    let saved = create_memo(move || !saving.with(|saving| saving.unsaved));
    let saves = create_memo(move || saving.with(|saving| saving.saves));
    let slow = create_signal(false);
    let dismissed = create_signal(false);
    let waiting = create_timer(move || {
        slow.set(true);
        None
    });
    create_effect(move || {
        saves.get();
        match saved.get() {
            true => untrack(|| {
                waiting.stop();
                slow.set(false);
                dismissed.set(false);
            }),
            false => untrack(|| {
                if !slow.get() {
                    waiting.restart(SLOW_SAVE);
                }
            }),
        }
    });
    let shown = create_memo(move || slow.get() && !dismissed.get());
    SlowSave { shown, dismissed }
}

/// The host's statistics as its debug panels show them: a row a measurement, under headings,
/// its performance first and then its memory, scrolling.
#[component]
fn StatisticsPanel(statistics: Signal<HostStatistics>, history: Signal<MemoryMinutes>) -> NodeId {
    let rows = create_memo(move || {
        statistics.with(|statistics| {
            let mut rows = statistics.performance.clone();
            rows.extend(statistics.memory.iter().cloned());
            rows
        })
    });
    let keys = create_memo(move || (0..rows.with(Vec::len)).collect::<Vec<_>>());
    let file = create_memo(move || {
        statistics.with(|statistics| {
            statistics
                .memory_file
                .as_ref()
                .map(|file| format!("Memory is written to {file}"))
                .unwrap_or_default()
        })
    });
    view! {
        <Scroll>
            <Frame padding=Space::Md>
                <List spacing=Space::Xs>
                    <Text style=TextStyle::TITLE>{STATISTICS_TITLE}</Text>
                    <MemoryGraph history={history} />
                    <StatisticLine
                        row={create_memo(|| Some(StatisticRow {
                            heading: true,
                            name: "Measurement".to_owned(),
                            current: "Latest".to_owned(),
                            average: "Average".to_owned(),
                            peak: "Peak".to_owned(),
                        }))}
                    />
                    <ForEach value={keys}>|index: usize| {
                        let row = create_memo(move || rows.with(|rows| rows.get(index).cloned()));
                        view! {
                            <StatisticLine row />
                        }
                    }</ForEach>
                    <Text size=TextSize::Xs color=TextColor::Muted>{file}</Text>
                </List>
            </Frame>
        </Scroll>
    }
}

/// The host's memory over time, as be-host's own panel graphs it: resident, anonymous,
/// shared and wasm memory a point a minute, each line in its own colour role, against a top of
/// the peak resident size rounded up to 64 MiB, a gap of more than two minutes breaking a line.
/// Nothing shows until a minute is measured.
#[component]
fn MemoryGraph(history: Signal<MemoryMinutes>) -> NodeId {
    let measured = create_memo(move || history.with(|history| !history.points.is_empty()));
    view! {
        <Show value={measured}>
            <MemoryDrawing history={history} />
        </Show>
    }
}

/// The graph itself, drawn each time the minutes or the theme change.
#[component]
fn MemoryDrawing(history: Signal<MemoryMinutes>) -> NodeId {
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::dynamic(move || {
        let history = history.get();
        let colors = [theme.accent.get(), theme.success.get(), theme.warning.get(), theme.danger.get()];
        let border = theme.border.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            draw_memory(painter, rect, &history, colors, border);
        }) as Draw
    });
    view! {
        <Frame height={Size::Custom(MEMORY_GRAPH_HEIGHT)}>
            <Drawing draw />
        </Frame>
    }
}

/// The memory graph's height, in points.
const MEMORY_GRAPH_HEIGHT: f32 = 60.0;

fn draw_memory(
    painter: &Painter,
    rect: Rect,
    history: &MemoryMinutes,
    colors: [beui::Color; 4],
    border: beui::Color,
) {
    painter.rect_stroke(rect, 0.0, 1.0, border);
    let Some(first) = history.points.first() else {
        return;
    };
    const STEP: u64 = 64 << 20;
    let peak = history.points.iter().map(|point| point.resident).max().unwrap_or(0);
    let top = peak.div_ceil(STEP).max(1) * STEP;
    let span = history.now.saturating_sub(first.at).max(60) as f32;
    let at = |point: &be_wasm_root_api::MemoryPoint, value: u64| {
        pos2(
            rect.left() + rect.width() * point.at.saturating_sub(first.at) as f32 / span,
            rect.bottom() - rect.height() * value as f32 / top as f32,
        )
    };
    let lines: [fn(&be_wasm_root_api::MemoryPoint) -> u64; 4] = [
        |point| point.resident,
        |point| point.anonymous,
        |point| point.shared,
        |point| point.wasm,
    ];
    for (line, color) in lines.into_iter().zip(colors).rev() {
        let mut path = beui::Path::new();
        let mut previous: Option<u64> = None;
        for point in &history.points {
            let spot = at(point, line(point));
            path = match previous {
                Some(before) if point.at.saturating_sub(before) <= 120 => path.line_to(spot),
                _ => path.move_to(spot),
            };
            previous = Some(point.at);
        }
        if let [only] = history.points.as_slice() {
            path = path.line_to(pos2(rect.right(), at(only, line(only)).y));
        }
        painter.stroke_path(
            &path,
            &beui::Stroke::new(1.5).join(beui::LineJoin::Round),
            color,
        );
    }
}

/// One row of the host's statistics: its name, and its latest value, average and peak, each
/// right-aligned in a column of its own.
#[component]
fn StatisticLine(row: Memo<Option<StatisticRow>>) -> NodeId {
    let cell = |read: fn(&StatisticRow) -> String| {
        create_memo(move || row.with(|row| row.as_ref().map(read).unwrap_or_default()))
    };
    let name = cell(|row| row.name.clone());
    let current = cell(|row| row.current.clone());
    let average = cell(|row| row.average.clone());
    let peak = cell(|row| row.peak.clone());
    let weight =
        create_memo(
            move || match row.with(|row| row.as_ref().is_some_and(|row| row.heading)) {
                true => Weight::Bold,
                false => Weight::Normal,
            },
        );
    view! {
        <List direction=Direction::Horizontal spacing=Space::Sm>
            <ListItem grow=1.0>
                <Text overflow=Overflow::Ellipsis size=TextSize::Xs weight={weight}>{name}</Text>
            </ListItem>
            <StatisticCell value={current} weight={weight} />
            <StatisticCell value={average} weight={weight} />
            <StatisticCell value={peak} weight={weight} />
        </List>
    }
}

/// One value of a row, right-aligned in its column.
#[component]
fn StatisticCell(value: Memo<String>, weight: Memo<Weight>) -> NodeId {
    view! {
        <Frame width=Size::Xl3>
            <Text size=TextSize::Xs family=Family::Mono weight={weight} align=TextAlign::End>
                {value}
            </Text>
        </Frame>
    }
}

/// The lock screen: whose session it is, a password field it focuses, why the last password
/// was refused, and what the host offers besides unlocking. What is typed is handed to the host
/// to check, and the field cleared as it is.
#[component]
fn LockCard(state: Signal<LockState>, shown_at: Signal<Duration>) -> NodeId {
    let password = create_signal(String::new());
    let user = create_memo(move || state.with(|state| state.user.clone()));
    let checking = create_memo(move || state.with(|state| state.checking));
    let refused = create_memo(move || state.with(|state| state.refused.clone()));
    let powers = create_memo(move || state.with(|state| state.power.clone()));
    let wait = retry_wait(state, shown_at);
    let slots = create_memo(move || (0..powers.with(Vec::len)).collect::<Vec<usize>>());
    let submit = move |typed: String| {
        if typed.is_empty() || checking.get_untracked() {
            return;
        }
        password.set(String::new());
        unlock(typed);
    };
    view! {
        <Frame width=Length::FILL height=Length::FILL align=Align2::CENTER padding=Space::Lg>
            <List spacing=Space::Md align=Align::Center>
                <Text style=TextStyle::TITLE>{user}</Text>
                <TextInput
                    value={password}
                    placeholder="Password"
                    label="Password"
                    kind={InputKind::Password}
                    disabled={checking}
                    focused=true
                    on_submit={submit}
                />
                <Show value={move || checking.get() && refused.with(Option::is_none)}>
                    <Text style=TextStyle::CAPTION>"Checking…"</Text>
                </Show>
                <Show value={refused}>|reason: ReadSignal<String>|
                    <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
                </Show>
                <Show value={wait}>|left: ReadSignal<String>|
                    <Text style=TextStyle::CAPTION>{left}</Text>
                </Show>
                <List direction=Direction::Horizontal spacing=Space::Xs>
                    <ForEach value={slots}>|index: usize| {
                        let action = create_memo(move || {
                            powers.with(|powers| powers.get(index).copied())
                        });
                        let label = create_memo(move || action.get().map(power_label).unwrap_or_default());
                        let glyph = create_memo(move || action.get().map(power_glyph));
                        view! {
                            <Button
                                icon_only=true
                                glyph
                                label
                                on_click={move || {
                                    if let Some(action) = action.get_untracked() {
                                        lock_power(action);
                                    }
                                }}
                            />
                        }
                    }</ForEach>
                </List>
            </List>
        </Frame>
    }
}

/// "Try again in N s" while the host makes the person wait after failed passwords (`retry-at`,
/// on the host's frame clock), counted down a second at a time by a timer on beui's clock, which
/// the root pins to the host's: `shown_at` is the host's frame time when the state was shown.
/// None once the wait is over.
fn retry_wait(state: Signal<LockState>, shown_at: Signal<Duration>) -> Memo<Option<String>> {
    let retry_at = create_memo(move || state.with(|state| state.retry_at));
    let left = create_signal(None::<u64>);
    // When the wait was last shown, on beui's clock and on the host's.
    let from = StoredCell::new((now(), Duration::ZERO));
    let tick = create_timer(move || {
        let at = retry_at.get_untracked()?;
        let (since, host) = from.get();
        let host_now = host + now().saturating_duration_since(since);
        let host_now = u64::try_from(host_now.as_nanos()).unwrap_or(u64::MAX);
        let seconds = at.saturating_sub(host_now).div_ceil(1_000_000_000);
        left.set((seconds > 0).then_some(seconds));
        (seconds > 0).then_some(Duration::from_secs(1))
    });
    create_effect(move || {
        retry_at.get();
        let host = shown_at.get();
        untrack(|| {
            from.set((now(), host));
            left.set(None);
            tick.restart(Duration::ZERO);
        });
    });
    create_memo(move || {
        left.get()
            .map(|seconds| format!("Try again in {seconds} s"))
    })
}

/// Hands `typed` to the host to check (`host.unlock`), keeping no copy: whether it unlocked the
/// session reaches the root as the lock's next change, and why it was refused with it.
fn unlock(typed: String) {
    let password = Password::from(typed);
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::unlock(password).await;
    });
}

/// Tells the host the person chose `action` on the lock screen.
fn lock_power(action: PowerAction) {
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::lock_power(action).await;
    });
}

fn power_label(action: PowerAction) -> String {
    match action {
        PowerAction::Suspend => "Suspend",
        PowerAction::Restart => "Restart",
        PowerAction::PowerOff => "Power off",
        PowerAction::LogOut => "Log out",
    }
    .to_owned()
}

fn power_glyph(action: PowerAction) -> Glyph {
    match action {
        PowerAction::Suspend => ICON_MOON,
        PowerAction::Restart => ICON_ROTATE_CCW,
        PowerAction::PowerOff => ICON_POWER,
        PowerAction::LogOut => ICON_LOG_OUT,
    }
    .to_owned()
}

/// The question of keeping display settings that just changed: Keep, or Revert. It counts down
/// on its clock, which the root pins to its host's frame clock, from `shown_at`, the host's
/// time it opened at, to `revert-at`, when the host reverts them itself; reaching it, it closes
/// answering not kept, a courtesy the host does not wait for. Escape reverts at once.
#[component]
fn KeepDisplayDialog(asked: DisplayQuestion, shown_at: Duration, done: Callback<Closed>) -> NodeId {
    let round = asked.round;
    let within = Duration::from_nanos(asked.revert_at).saturating_sub(shown_at);
    let deadline = now() + within;
    let left = create_signal(within);
    let ticking = create_timer(move || {
        let remaining = deadline.saturating_duration_since(now());
        left.set(remaining);
        if remaining.is_zero() {
            done.call(Closed::Reverted(round));
            return None;
        }
        Some(remaining.min(Duration::from_secs(1)))
    });
    ticking.start(within.min(Duration::from_secs(1)));
    let message = create_memo(move || match left.get().as_secs_f64().ceil() as u64 {
        1 => "Reverting in 1 second.".to_owned(),
        seconds => format!("Reverting in {seconds} seconds."),
    });
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                done.call(Closed::Reverted(round));
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                kind=DialogKind::Alert
                title=KEEP_DISPLAY_TITLE
                scrolls=false
                actions={vec![
                    DialogAction::primary("Keep", move || done.call(Closed::Kept(round))),
                    DialogAction::cancel("Revert"),
                ]}
                on_dismiss={move || done.call(Closed::Reverted(round))}
            >
                <Text style=TextStyle::PARAGRAPH>{message}</Text>
            </DialogBody>
        </Frame>
    }
}

/// The list of the block stack's unsent edits, as `core` shows them: each block's name and why
/// its edits were set aside, with Restore, which sends them again, and Discard, which asks
/// first, in the list's place. A block that leaves the list takes its question with it. Escape
/// cancels the question, or else closes the list.
#[component]
fn UnsentDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    // A memo per area of the core's view: this one changes only when the unsent edits do.
    let edits = create_memo(move || core.with(|view| view.unsent.clone()));
    let discarding = create_signal(None::<[u8; 16]>);
    let asked = create_memo(move || {
        let block = discarding.get()?;
        edits.with(|edits| edits.iter().find(|edit| edit.block == block).cloned())
    });
    let asks = create_memo(move || asked.with(Option::is_some));
    let question = create_memo(move || {
        let name = asked.with(|edit| edit.as_ref().map(edit_name).unwrap_or_default());
        format!(
            "Discard the edits to {name}? They are kept only on this device, and cannot be brought back."
        )
    });
    let discard = move || {
        if let Some(edit) = asked.get() {
            ask_core(CoreCommand::DiscardUnsent(edit.block));
        }
        discarding.set(None);
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                match asks.get_untracked() {
                    true => discarding.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <Show
                value={asks}
                fallback={move || view! {
                    <DialogBody
                        title=UNSENT_TITLE
                        actions={vec![DialogAction::cancel("Close")]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <List spacing=Space::Md>
                            <ForEach
                                value={edits}
                                key={|edit: &UnsentEdit| edit.block}
                                fallback={|| view! {
                                    <Text style=TextStyle::CAPTION>"No unsent edits."</Text>
                                }}
                            >|edit: ReadSignal<UnsentEdit>| {
                                let block = edit.with_untracked(|edit| edit.block);
                                let name = create_memo(move || edit.with(edit_name));
                                let detail = create_memo(move || edit.with(edit_detail));
                                view! {
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=Space::Md
                                    >
                                        <ListItem grow=1.0>
                                            <List spacing=Space::Xs2>
                                                <Text>{name}</Text>
                                                <Text style=TextStyle::CAPTION>{detail}</Text>
                                            </List>
                                        </ListItem>
                                        <Button
                                            label="Restore"
                                            variant=ButtonVariant::Primary
                                            disabled={move || !edit.get().offerable}
                                            on_click={move || ask_core(CoreCommand::RestoreUnsent(block))}
                                        />
                                        <Button
                                            icon_only=true
                                            glyph=ICON_TRASH
                                            label="Discard"
                                            on_click={move || discarding.set(Some(block))}
                                        />
                                    </List>
                                }
                            }</ForEach>
                        </List>
                    </DialogBody>
                }}
            >
                <DialogBody
                    kind=DialogKind::Alert
                    title=DISCARD_UNSENT_TITLE
                    scrolls=false
                    actions={vec![
                        DialogAction::destructive("Discard", discard),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={move || discarding.set(None)}
                >
                    <Text style=TextStyle::PARAGRAPH>{question}</Text>
                </DialogBody>
            </Show>
        </Frame>
    }
}

/// What the block stack waits for the person to do: the accounts, the organizations or the
/// sessions, each splitting its own memo off the one view.
#[component]
fn Onboarding(
    core: Signal<CoreView>,
    want: Callback<Asking>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let waits = create_memo(move || core.with(|view| view.waits_for.clone()));
    let recovery = create_memo(move || waits.get() == Some(CoreWait::Recovery));
    let unlock = create_memo(move || waits.get() == Some(CoreWait::Unlock));
    let failure = create_memo(move || waits.get() == Some(CoreWait::Failure));
    let accounts = create_memo(move || waits.get() == Some(CoreWait::Account));
    let organizations = create_memo(move || waits.get() == Some(CoreWait::Organization));
    let sessions = create_memo(move || waits.get() == Some(CoreWait::Session));
    view! {
        <Frame width=Length::FILL height=Length::FILL fill=ColorRole::Background>
            <List spacing=Space::None>
                <Show value={accounts}>
                    <ListItem grow=1.0>
                        <AccountsCard core={core} want={want} />
                    </ListItem>
                </Show>
                <Show value={organizations}>
                    <ListItem grow=1.0>
                        <OrganizationsCard core={core} />
                    </ListItem>
                </Show>
                <Show value={sessions}>
                    <ListItem grow=1.0>
                        <SessionsCard core={core} />
                    </ListItem>
                </Show>
                <Show value={recovery}>
                    <ListItem grow=1.0>
                        <RecoveryCard core={core} secret={secret} pending={pending} ask={ask} />
                    </ListItem>
                </Show>
                <Show value={unlock}>
                    <ListItem grow=1.0>
                        <UnlockCard core={core} secret={secret} pending={pending} ask={ask} />
                    </ListItem>
                </Show>
                <Show value={failure}>
                    <ListItem grow=1.0>
                        <FailureCard core={core} />
                    </ListItem>
                </Show>
            </List>
        </Frame>
    }
}

/// The recovery phrase, while the person writes it down: its words, asked for as it shows and
/// held only while it does, numbered; a field for the words at the checked positions, typed
/// back as a secret and cleared; Confirm, New phrase, and Keep the old one while replacing.
#[component]
fn RecoveryCard(
    core: Signal<CoreView>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.recovery.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let replacing = create_memo(move || area.with(|area| area.replacing));
    ask.call(SecretCommand::RecoveryPhrase);
    // The one copy of the words it shows beside the phrase itself.
    let words = create_memo(move || {
        secret.with(|secret| {
            secret
                .as_ref()
                .and_then(Held::phrase)
                .map(|phrase| {
                    phrase
                        .split_whitespace()
                        .enumerate()
                        .map(|(at, word)| format!("{} {word}", at + 1))
                        .collect::<Vec<_>>()
                        .join("   ")
                })
                .unwrap_or_default()
        })
    });
    let asked = create_memo(move || {
        area.with(|area| {
            let positions: Vec<String> = area.checked.iter().map(u32::to_string).collect();
            format!(
                "Type words {} to show you wrote it down.",
                positions.join(", ")
            )
        })
    });
    let typed = create_signal(String::new());
    let confirm = move || {
        let words = typed.get_untracked();
        typed.set(String::new());
        if !words.trim().is_empty() {
            ask_core(CoreCommand::ConfirmRecovery(Password::from(words)));
        }
    };
    let renew = move || {
        // The host makes the new phrase with the command, before the request after it.
        ask_core(CoreCommand::NewRecoveryPhrase);
        ask.call(SecretCommand::RecoveryPhrase);
    };
    view! {
        <be_host_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{RECOVERY_TITLE}</Text>
            <Text style=TextStyle::CAPTION>
                "Write these words down and keep them safe: they are the only way back to your blocks if you lose this device."
            </Text>
            <Text>{words}</Text>
            <Text style=TextStyle::CAPTION>{asked}</Text>
            <be_host_accounts_ui::ErrorText text={error} />
            <TextInput
                value={typed}
                label="Words"
                placeholder="Words"
                kind={InputKind::Password}
                on_submit={move |_: String| confirm()}
            />
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label="Confirm"
                    variant=ButtonVariant::Primary
                    disabled={busy}
                    on_click={confirm}
                />
                <Button
                    label="New phrase"
                    variant=ButtonVariant::Secondary
                    disabled={busy}
                    on_click={renew}
                />
                <Show value={replacing}>
                    <Button
                        label="Keep the old one"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::CancelRecovery)}
                    />
                </Show>
            </List>
        </be_host_accounts_ui::Column>
    }
}

/// Unlocking the account's keys on this device: its recovery phrase, typed as a secret and
/// cleared; or pairing with a device that holds them, whose code, asked for, shows until it is
/// withdrawn.
#[component]
fn UnlockCard(
    core: Signal<CoreView>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.unlock.clone()));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let lead = create_memo(move || {
        area.with(|area| format!("The keys of {} are not on this device yet.", area.account))
    });
    let code = create_memo(move || {
        secret.with(|secret| secret.as_ref().and_then(Held::code).map(str::to_owned))
    });
    let pairing = create_memo(move || code.with(Option::is_some));
    let unpaired = create_memo(move || !pairing.get());
    let code_line = create_memo(move || {
        let account = area.with(|area| area.account.clone());
        code.get()
            .map(|code| format!("Enter {code} on a device signed in as {account}."))
            .unwrap_or_default()
    });
    let phrase = create_signal(String::new());
    let unlock = move || {
        let typed = phrase.get_untracked();
        phrase.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::UnlockWithPhrase(Password::from(typed)));
        }
    };
    view! {
        <be_host_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{UNLOCK_TITLE}</Text>
            <Text style=TextStyle::CAPTION>{lead}</Text>
            <be_host_accounts_ui::ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                <ListItem grow=1.0>
                    <TextInput
                        value={phrase}
                        label="Recovery phrase"
                        placeholder="Recovery phrase"
                        kind={InputKind::Password}
                        on_submit={move |_: String| unlock()}
                    />
                </ListItem>
                <Button label="Unlock" variant=ButtonVariant::Primary on_click={unlock} />
            </List>
            <Show value={unpaired}>
                <Button
                    label="Pair with another device"
                    variant=ButtonVariant::Secondary
                    disabled={pending}
                    on_click={move || ask.call(SecretCommand::PairingCode)}
                />
            </Show>
            <Show value={pairing}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <Text>{code_line}</Text>
                    </ListItem>
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            secret.set(None);
                            ask_core(CoreCommand::CancelPairing);
                        }}
                    />
                </List>
            </Show>
        </be_host_accounts_ui::Column>
    }
}

/// What the block stack's failure leaves the person to do: start it again, delete this
/// device's copy of the blocks (asked first, saying what is lost), or quit.
#[component]
fn FailureCard(core: Signal<CoreView>) -> NodeId {
    let failure = create_memo(move || core.with(|view| view.failure.clone()));
    let message = create_memo(move || {
        failure.with(|it| it.as_ref().map(|it| it.message.clone()).unwrap_or_default())
    });
    let asking = create_signal(false);
    let calm = create_memo(move || !asking.get());
    let question = create_memo(move || {
        let unsaved = failure.with(|it| it.as_ref().map_or(0, |it| it.unsaved));
        match unsaved {
            0 => "Delete this device's copy of your blocks? They download again from the server."
                .to_owned(),
            1 => "Delete this device's copy of your blocks? 1 change not saved is lost.".to_owned(),
            unsaved => format!(
                "Delete this device's copy of your blocks? {unsaved} changes not saved are lost."
            ),
        }
    });
    view! {
        <be_host_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{FAILURE_TITLE}</Text>
            <Text style=TextStyle::PARAGRAPH>{message}</Text>
            <Show value={calm}>
                <List direction=Direction::Horizontal spacing=Space::Md>
                    <Button
                        label="Restart"
                        variant=ButtonVariant::Primary
                        on_click={|| ask_core(CoreCommand::Restart)}
                    />
                    <Button
                        label="Delete local copy"
                        variant=ButtonVariant::Secondary
                        on_click={move || asking.set(true)}
                    />
                    <Button
                        label="Quit"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::Exit)}
                    />
                </List>
            </Show>
            <Show value={asking}>
                <List spacing=Space::Md>
                    <Text style=TextStyle::PARAGRAPH>{question}</Text>
                    <List direction=Direction::Horizontal spacing=Space::Md>
                        <Button
                            label="Keep"
                            variant=ButtonVariant::Secondary
                            on_click={move || asking.set(false)}
                        />
                        <Button
                            label="Delete"
                            variant=ButtonVariant::Danger
                            on_click={move || {
                                asking.set(false);
                                ask_core(CoreCommand::DeleteClientDatabase);
                            }}
                        />
                    </List>
                </List>
            </Show>
        </be_host_accounts_ui::Column>
    }
}

/// Another device's request to pair, the host's question: the code that device shows, typed as
/// a secret and cleared, to approve it, which stays until the host no longer lists the request;
/// or Not now, after which the root asks about it no more.
#[component]
fn PairingDialog(core: Signal<CoreView>, id: u64, done: Callback<Closed>) -> NodeId {
    let request = create_memo(move || {
        core.with(|view| {
            view.pairing
                .iter()
                .find(|request| request.id == id)
                .cloned()
        })
    });
    let lead = create_memo(move || {
        request.with(|request| {
            let device = request
                .as_ref()
                .map(|it| it.device.clone())
                .unwrap_or_default();
            format!("{device} asks to pair. Type the code it shows.")
        })
    });
    let refused = create_memo(move || request.with(|request| request.as_ref()?.refused.clone()));
    let code = create_signal(String::new());
    let approve = move || {
        let typed = code.get_untracked();
        code.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::ApprovePairing(PairingApproval {
                id,
                code: Password::from(typed),
            }));
        }
    };
    escape_dismisses(done);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                title=PAIRING_TITLE
                actions={vec![
                    DialogAction::cancel("Not now"),
                    DialogAction::primary("Approve", approve),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::CAPTION>{lead}</Text>
                <be_host_accounts_ui::ErrorText text={refused} />
                <TextInput
                    value={code}
                    label="Code"
                    placeholder="Code"
                    on_submit={move |_: String| approve()}
                />
            </DialogBody>
        </Frame>
    }
}

/// The account's organizations: each to open, in its last session or choosing one; an
/// invitation opened, to join or put aside; creating one; opening an invitation's link, which
/// the field hands over as a secret and clears; and switching account.
#[component]
fn OrganizationsCard(core: Signal<CoreView>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.organizations.clone()));
    let listed = create_memo(move || area.with(|area| area.listed.clone()));
    let loading = create_memo(move || listed.get() == OrganizationsListed::Loading);
    let failed = create_memo(move || listed.get() == OrganizationsListed::Failed);
    let rows = create_memo(move || match listed.get() {
        OrganizationsListed::Loaded(rows) => rows,
        _ => Vec::new(),
    });
    let busy = create_memo(move || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let joining = create_memo(move || area.with(|area| area.joining.is_some()));
    let invited = create_memo(move || {
        area.with(|area| {
            area.joining
                .as_ref()
                .map(|invitation| {
                    let lead = format!(
                        "{} invited you to join {}",
                        invitation.inviter, invitation.organization
                    );
                    match &invitation.problem {
                        Some(problem) => format!("{lead}. {problem}"),
                        None => lead,
                    }
                })
                .unwrap_or_default()
        })
    });
    let cannot_join = create_memo(move || {
        busy.get() || !area.with(|area| area.joining.as_ref().is_some_and(|it| it.can_join))
    });
    let name = create_signal(String::new());
    let link = create_signal(String::new());
    let create = move || {
        let named = name.get_untracked().trim().to_owned();
        if !named.is_empty() {
            name.set(String::new());
            ask_core(CoreCommand::CreateOrganization(named));
        }
    };
    let open_link = move || {
        // The link is a bearer secret: shown as typed, so the person sees what they pasted,
        // it leaves the field as it is handed over.
        let typed = link.get_untracked();
        link.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::OpenInvitation(Password::from(typed)));
        }
    };
    view! {
        <be_host_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{ORGANIZATIONS_TITLE}</Text>
            <Show value={loading}>
                <Text style=TextStyle::CAPTION>"Loading organizations…"</Text>
            </Show>
            <Show value={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <Text style=TextStyle::CAPTION>
                            "The organizations could not be listed."
                        </Text>
                    </ListItem>
                    <Button
                        label="Try again"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::ReloadOrganizations)}
                    />
                </List>
            </Show>
            <ForEach
                value={rows}
                key={|row: &OrganizationRow| row.id}
            >|row: ReadSignal<OrganizationRow>| {
                let id = row.with_untracked(|row| row.id);
                let name = create_memo(move || row.with(|row| row.name.clone()));
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <ListItem grow=1.0>
                            <Text>{name}</Text>
                        </ListItem>
                        <Button
                            label="Sessions"
                            variant=ButtonVariant::Secondary
                            on_click={move || ask_core(CoreCommand::ChooseSessionIn(id))}
                        />
                        <Button
                            label="Open"
                            variant=ButtonVariant::Primary
                            on_click={move || ask_core(CoreCommand::OpenOrganization(id))}
                        />
                    </List>
                }
            }</ForEach>
            <Show value={joining}>
                <Card>
                    <List spacing=Space::Md>
                        <Text>{invited}</Text>
                        <List
                            direction=Direction::Horizontal
                            justify=Justify::End
                            spacing=Space::Md
                        >
                            <Button
                                label="Not now"
                                variant=ButtonVariant::Secondary
                                on_click={|| ask_core(CoreCommand::CancelJoin)}
                            />
                            <Button
                                label="Join"
                                variant=ButtonVariant::Primary
                                disabled={cannot_join}
                                on_click={|| ask_core(CoreCommand::JoinInvited)}
                            />
                        </List>
                    </List>
                </Card>
            </Show>
            <be_host_accounts_ui::ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                <ListItem grow=1.0>
                    <TextInput
                        value={name}
                        label="New organization"
                        placeholder="New organization"
                        on_submit={move |_: String| create()}
                    />
                </ListItem>
                <Button label="Create" disabled={busy} on_click={create} />
            </List>
            <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                <ListItem grow=1.0>
                    <TextInput
                        value={link}
                        label="Invitation link"
                        placeholder="Invitation link"
                        on_submit={move |_: String| open_link()}
                    />
                </ListItem>
                <Button label="Open invitation" disabled={busy} on_click={open_link} />
            </List>
            <Button
                label="Switch account"
                variant=ButtonVariant::Secondary
                on_click={|| ask_core(CoreCommand::SwitchAccount)}
            />
        </be_host_accounts_ui::Column>
    }
}

/// The organization's sessions: those of the type this device opens, or of every type, each to
/// open; a new one; this device's profiles, each to use, and a new one; and Close, back in the
/// session open.
#[component]
fn SessionsCard(core: Signal<CoreView>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.sessions.clone()));
    let loading = create_memo(move || !area.with(|area| area.loaded));
    // Which types it lists is the root's own choice, not the host's.
    let every = create_signal(false);
    let shown = create_memo(move || {
        let every = every.get();
        area.with(|area| {
            area.sessions
                .iter()
                .filter(|session| every || session.session_type == area.session_type)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let profiles = create_memo(move || area.with(|area| area.profiles.clone()));
    let new_label = create_memo(move || {
        area.with(|area| {
            let name = area
                .session_kinds
                .iter()
                .find(|kind| kind.id == area.session_type)
                .map(|kind| kind.name.clone())
                .unwrap_or_default();
            format!("New {name} session")
        })
    });
    let every_label = create_memo(move || match every.get() {
        true => "Show this type only".to_owned(),
        false => "Show every type".to_owned(),
    });
    view! {
        <be_host_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{SESSIONS_TITLE}</Text>
            <Show value={loading}>
                <Text style=TextStyle::CAPTION>"Loading sessions and profiles…"</Text>
            </Show>
            <ForEach
                value={shown}
                key={|session: &SessionRow| session.id}
            >|session: ReadSignal<SessionRow>| {
                let id = session.with_untracked(|session| session.id);
                let label = create_memo(move || {
                    session.with(|session| match session.current {
                        true => format!("{} (last used)", session.name),
                        false => session.name.clone(),
                    })
                });
                view! {
                    <Button
                        label={label}
                        variant=ButtonVariant::Secondary
                        on_click={move || ask_core(CoreCommand::OpenSession(id))}
                    />
                }
            }</ForEach>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label={new_label}
                    variant=ButtonVariant::Primary
                    on_click={move || ask_core(CoreCommand::NewSession(area.with_untracked(|area| area.session_type)))}
                />
                <Button
                    label={every_label}
                    variant=ButtonVariant::Secondary
                    on_click={move || every.set(!every.get_untracked())}
                />
            </List>
            <Text weight=Weight::Bold>"Profiles"</Text>
            <ForEach
                value={profiles}
                key={|profile: &ProfileRow| profile.id}
            >|profile: ReadSignal<ProfileRow>| {
                let id = profile.with_untracked(|profile| profile.id);
                let label = create_memo(move || {
                    profile.with(|profile| match profile.current {
                        true => format!("{} (in use)", profile.name),
                        false => profile.name.clone(),
                    })
                });
                view! {
                    <Button
                        label={label}
                        variant=ButtonVariant::Secondary
                        on_click={move || ask_core(CoreCommand::UseProfile(id))}
                    />
                }
            }</ForEach>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label="New profile"
                    variant=ButtonVariant::Secondary
                    on_click={|| ask_core(CoreCommand::NewProfile)}
                />
                <Button
                    label="Close"
                    variant=ButtonVariant::Secondary
                    on_click={|| ask_core(CoreCommand::CloseSessions)}
                />
            </List>
        </be_host_accounts_ui::Column>
    }
}

/// The accounts this device keeps, as be-host-accounts-ui shows them: opening one, choosing its
/// organization, removing it (asking first for one kept on this device alone, whose blocks go
/// with it), or adding one, which `want` opens a dialog for.
#[component]
fn AccountsCard(core: Signal<CoreView>, want: Callback<Asking>) -> NodeId {
    // A memo per area of the core's view: these change only when the accounts do.
    let shown = create_memo(move || core.with(|view| view.accounts.clone()));
    let accounts = create_memo(move || {
        shown.with(|accounts| accounts.iter().map(account_row).collect::<Vec<_>>())
    });
    let error = create_memo(move || core.with(|view| view.account_error.clone()));
    let forget = move |key: String| {
        let local = shown.with_untracked(|accounts| {
            accounts
                .iter()
                .find(|account| account.key == key)
                .filter(|account| account.server.is_none())
                .map(|account| account.name.clone())
        });
        match local {
            Some(name) => want.call(Asking::ForgetAccount { key, name }),
            None => ask_core(CoreCommand::ForgetAccount(key)),
        }
    };
    view! {
        <Frame width=Length::FILL height=Length::FILL fill=ColorRole::Background>
            <be_host_accounts_ui::AccountsScreen
                accounts
                error
                on_add={move || want.call(Asking::AddAccount { resumed: false })}
                on_open={|key: String| ask_core(CoreCommand::OpenAccount(key))}
                on_choose_organization={|key: String| ask_core(CoreCommand::ChooseOrganization(key))}
                on_log_out={forget}
            />
        </Frame>
    }
}

/// An account as be-host-accounts-ui shows it.
fn account_row(account: &AccountRow) -> be_host_accounts_ui::AccountRow {
    be_host_accounts_ui::AccountRow {
        key: account.key.clone(),
        name: account.name.clone(),
        email: account.email.clone(),
        server: account
            .server
            .clone()
            .unwrap_or_else(|| "On this device".to_owned()),
        local: account.server.is_none(),
        has_last_organization: account.has_last_organization,
        current: account.current,
        companies: account.organizations.clone(),
    }
}

/// Where the person chose to keep the account they add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keeping {
    Server,
    Device,
}

/// Adding an account: where to keep it, then a server's address and the gateway token it may
/// need, or a name for one on this device; then, while the host signs in, the code the
/// browser's page asks for, with Open browser and Check now. It closes once an account it did
/// not list when it opened (`known`) is added. Escape steps back to choosing where, or closes.
#[component]
fn AddAccountDialog(core: Signal<CoreView>, known: Vec<String>, done: Callback<Closed>) -> NodeId {
    let signing = create_memo(move || core.with(|view| view.signing_in.clone()));
    let error = create_memo(move || core.with(|view| view.account_error.clone()));
    let added = create_memo(move || {
        core.with(|view| {
            view.accounts
                .iter()
                .any(|account| !known.contains(&account.key))
        })
    });
    create_effect(move || {
        if added.get() {
            done.call(Closed::Added);
        }
    });
    // A sign-in it showed that ends with no error signed in: an account signed in to again
    // (reauthenticating) is no account added, but closes it all the same.
    let showed = StoredCell::new(false);
    create_effect(move || {
        let signing = core.with(|view| view.signing_in.is_some());
        let failed = core.with(|view| view.account_error.is_some());
        match signing {
            true => showed.set(true),
            false if showed.get() && !failed => done.call(Closed::Added),
            false => {}
        }
    });
    let keeping = create_signal(None::<Keeping>);
    let url = create_signal(String::new());
    let gateway = create_signal(String::new());
    let name = create_signal(String::new());
    let waiting = create_memo(move || signing.with(Option::is_some));
    let browser = create_memo(move || {
        signing.with(|signing| signing.as_ref().and_then(|signing| signing.browser.clone()))
    });
    let has_browser = create_memo(move || browser.with(Option::is_some));
    let connecting = create_memo(move || waiting.get() && !has_browser.get());
    let choosing = create_memo(move || !waiting.get() && keeping.get().is_none());
    let to_server = create_memo(move || !waiting.get() && keeping.get() == Some(Keeping::Server));
    let on_device = create_memo(move || !waiting.get() && keeping.get() == Some(Keeping::Device));
    let code = create_memo(move || {
        browser.with(|browser| {
            browser
                .as_ref()
                .map(|browser| format!("Enter {} on the page that opens.", browser.code))
                .unwrap_or_default()
        })
    });
    let connect = move || {
        let address = url.get_untracked().trim().to_owned();
        if address.is_empty() {
            return;
        }
        // The token leaves the field as it is handed over, as the lock screen's password does.
        let typed = gateway.get_untracked();
        gateway.set(String::new());
        let gateway = (!typed.trim().is_empty()).then(|| Password::from(typed));
        ask_core(CoreCommand::AddAccount(NewAccount::Server(ServerAccount {
            url: address,
            gateway,
        })));
    };
    let create = move || {
        let name = name.get_untracked().trim().to_owned();
        if !name.is_empty() {
            ask_core(CoreCommand::AddAccount(NewAccount::Local(name)));
        }
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                match keeping.get_untracked().is_some() && !waiting.get_untracked() {
                    true => keeping.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    let dismiss = move || done.call(Closed::Dismissed);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={choosing}>
                    <DialogBody
                        title=ADD_ACCOUNT_TITLE
                        actions={vec![DialogAction::cancel("Cancel")]}
                        on_dismiss={dismiss}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <List direction=Direction::Horizontal spacing=Space::Md>
                            <Button
                                label="Connect to a server"
                                variant=ButtonVariant::Secondary
                                on_click={move || keeping.set(Some(Keeping::Server))}
                            />
                            <Button
                                label="On this device"
                                variant=ButtonVariant::Secondary
                                on_click={move || keeping.set(Some(Keeping::Device))}
                            />
                        </List>
                    </DialogBody>
                </Show>
                <Show value={to_server}>
                    <DialogBody
                        title="Connect to server"
                        actions={vec![
                            DialogAction::secondary("Back", move || keeping.set(None)),
                            DialogAction::cancel("Cancel"),
                            DialogAction::primary("Continue", connect)
                                .disabled(move || url.with(|url| url.trim().is_empty())),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <TextInput
                            value={url}
                            label="Server address"
                            focused=true
                            on_submit={move |_: String| connect()}
                        />
                        <TextInput
                            value={gateway}
                            label="Gateway token (optional)"
                            kind={InputKind::Password}
                            on_submit={move |_: String| connect()}
                        />
                    </DialogBody>
                </Show>
                <Show value={on_device}>
                    <DialogBody
                        title="Create local account"
                        actions={vec![
                            DialogAction::secondary("Back", move || keeping.set(None)),
                            DialogAction::cancel("Cancel"),
                            DialogAction::primary("Create account", create)
                                .disabled(move || name.with(|name| name.trim().is_empty())),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <TextInput
                            value={name}
                            label="Your name"
                            focused=true
                            on_submit={move |_: String| create()}
                        />
                    </DialogBody>
                </Show>
                <Show value={has_browser}>
                    <DialogBody
                        title=SIGN_IN_TITLE
                        actions={vec![
                            DialogAction::cancel("Cancel"),
                            DialogAction::secondary("Check now", || ask_core(CoreCommand::CheckSignIn)),
                            DialogAction::primary("Open browser", || ask_core(CoreCommand::OpenSignInPage)),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <Text style=TextStyle::PARAGRAPH>{code}</Text>
                    </DialogBody>
                </Show>
                <Show value={connecting}>
                    <DialogBody
                        title=SIGN_IN_TITLE
                        actions={vec![DialogAction::cancel("Cancel")]}
                        on_dismiss={dismiss}
                    >
                        <Text style=TextStyle::PARAGRAPH>"Connecting…"</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// The question before removing an account kept on this device alone, `name`, whose blocks go
/// with it: Remove, which asks the block stack to forget it, or Keep. Escape keeps it.
#[component]
fn ForgetAccountDialog(key: String, name: String, done: Callback<Closed>) -> NodeId {
    let remove = move || {
        ask_core(CoreCommand::ForgetAccount(key.clone()));
        done.call(Closed::Dismissed);
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                done.call(Closed::Dismissed);
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                kind=DialogKind::Alert
                title=FORGET_ACCOUNT_TITLE
                scrolls=false
                actions={vec![
                    DialogAction::destructive("Remove", remove),
                    DialogAction::cancel("Keep"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>
                    {format!("Remove {name}? Its blocks are kept only on this device, and go with it.")}
                </Text>
            </DialogBody>
        </Frame>
    }
}

/// The host's question of signing in again to an account whose sign-in ended: Continue in
/// browser, which starts the sign-in the dialog adding an account then shows; Log out, which
/// forgets the account; or closing it, which leaves the account for the accounts, as be-host's
/// own does.
#[component]
fn ReauthDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let asked = create_memo(move || core.with(|view| view.reauthenticating.clone()));
    let lead = create_memo(move || {
        asked.with(|asked| {
            let name = asked
                .as_ref()
                .map(|asked| asked.name.as_str())
                .unwrap_or_default();
            format!("{name}'s sign-in ended. Sign in again to go on.")
        })
    });
    let error = create_memo(move || asked.with(|asked| asked.as_ref()?.error.clone()));
    let busy = create_memo(move || asked.with(|asked| asked.as_ref().is_some_and(|it| it.busy)));
    let log_out = move || {
        if let Some(key) = asked.with_untracked(|asked| asked.as_ref().map(|it| it.key.clone())) {
            ask_core(CoreCommand::ForgetAccount(key));
        }
    };
    escape_dismisses(done);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                title=REAUTH_TITLE
                actions={vec![
                    DialogAction::secondary("Log out", log_out),
                    DialogAction::primary("Continue in browser", || ask_core(CoreCommand::Reauthenticate))
                        .busy(busy),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{lead}</Text>
                <be_host_accounts_ui::ErrorText text={error} />
            </DialogBody>
        </Frame>
    }
}

/// The host's question before discarding the changes not saved to this device yet, for
/// `reason`: discarding them, or keeping them (Escape, or closing it).
#[component]
fn DiscardDialog(reason: DiscardReason, done: Callback<Closed>) -> NodeId {
    let (message, button) = match reason {
        DiscardReason::SwitchOrganization => (
            "Switching organizations discards changes not saved on this device yet. Changes waiting to sync are kept.",
            "Discard and switch",
        ),
        DiscardReason::Close => (
            "Closing discards changes not saved on this device yet. Changes waiting to sync are kept.",
            "Discard and close",
        ),
    };
    escape_dismisses(done);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                kind=DialogKind::Alert
                title=DISCARD_TITLE
                scrolls=false
                actions={vec![
                    DialogAction::destructive(button, move || done.call(Closed::Confirmed)),
                    DialogAction::cancel("Keep"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{message}</Text>
            </DialogBody>
        </Frame>
    }
}

/// The organization's invitations: each with how it stands, its link again for one of the
/// person's own still open, and Revoke while open; and New link, which makes one. A link comes
/// back once, as the answer to the request that made it (`ask`, `make-secret`), and `link`
/// holds it only while it shows: Done, or closing the dialog, drops it, zeroed. While a request
/// is `pending` it asks for no other, so the answer it shows is the latest request's.
#[component]
fn InvitationsDialog(
    core: Signal<CoreView>,
    link: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
    done: Callback<Closed>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.invitations.clone()));
    let rows = create_memo(move || area.with(|area| area.rows.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy || !area.listed));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let showing = create_memo(move || link.with(Option::is_some));
    let listing = create_memo(move || !showing.get());
    // What it shows of the link, the one copy it keeps beside the link itself.
    let shown = create_memo(move || {
        link.with(|link| {
            link.as_ref()
                .and_then(Held::link)
                .unwrap_or_default()
                .to_owned()
        })
    });
    let copy = move || {
        if let Some(text) =
            link.with_untracked(|link| link.as_ref().and_then(Held::link).map(str::to_owned))
        {
            be_wasm_ops::guest::spawn(async move {
                let item = be_lib_ui_types::ClipItem::text(text);
                let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
            });
        }
    };
    escape_dismisses(done);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=INVITATIONS_TITLE
                        actions={vec![
                            DialogAction::cancel("Close"),
                            DialogAction::primary("New link", move || ask.call(SecretCommand::InviteMember))
                                .busy(busy)
                                .disabled(move || pending.get()),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <ForEach
                            value={rows}
                            key={|row: &InvitationRow| row.id}
                            fallback={|| view! {
                                <Text style=TextStyle::CAPTION>"No invitations yet."</Text>
                            }}
                        >|row: ReadSignal<InvitationRow>| {
                            let id = row.with_untracked(|row| row.id);
                            let title = create_memo(move || row.with(invitation_title));
                            let open = create_memo(move || {
                                row.with(|row| matches!(row.state, InvitationState::Waiting | InvitationState::Opened))
                            });
                            let has_link = create_memo(move || row.with(|row| row.has_link));
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <Text style=TextStyle::CAPTION>{title}</Text>
                                    </ListItem>
                                    <Show value={has_link}>
                                        <Button
                                            label="Link"
                                            variant=ButtonVariant::Secondary
                                            disabled={pending}
                                            on_click={move || ask.call(SecretCommand::InvitationLink(id))}
                                        />
                                    </Show>
                                    <Show value={open}>
                                        <Button
                                            label="Revoke"
                                            variant=ButtonVariant::Secondary
                                            on_click={move || ask_core(CoreCommand::RevokeInvitation(id))}
                                        />
                                    </Show>
                                </List>
                            }
                        }</ForEach>
                    </DialogBody>
                </Show>
                <Show value={showing}>
                    <DialogBody
                        title="Invitation link"
                        actions={vec![
                            DialogAction::secondary("Done", move || link.set(None)),
                            DialogAction::primary("Copy", copy),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::CAPTION>
                            "Whoever has this link can join the organization."
                        </Text>
                        <Text>{shown}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
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

/// The organization's members: each with their email and role, and, but for the person
/// themselves and agents, a change of role (administrator or editor) and Remove, which asks
/// first in the list's place. A member who leaves the list takes the question with them.
#[component]
fn MembersDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.members.clone()));
    let members = create_memo(move || area.with(|area| area.members.clone()));
    let busy = create_memo(move || area.with(|area| area.busy || !area.listed));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let removing = create_signal(None::<[u8; 16]>);
    let asked = create_memo(move || {
        let account = removing.get()?;
        members.with(|members| {
            members
                .iter()
                .find(|member| member.account == account)
                .cloned()
        })
    });
    let asks = create_memo(move || asked.with(Option::is_some));
    let listing = create_memo(move || !asks.get());
    let question = create_memo(move || {
        let name = asked.with(|member| {
            member
                .as_ref()
                .map(|member| member.name.clone())
                .unwrap_or_default()
        });
        format!("Remove {name} from the organization? They lose access to its blocks.")
    });
    let remove = move || {
        if let Some(member) = asked.get_untracked() {
            ask_core(CoreCommand::RemoveMember(member.account));
        }
        removing.set(None);
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                match asks.get_untracked() {
                    true => removing.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=MEMBERS_TITLE
                        actions={vec![DialogAction::cancel("Close")]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <ForEach
                            value={members}
                            key={|member: &MemberRow| member.account}
                        >|member: ReadSignal<MemberRow>| {
                            let account = member.with_untracked(|member| member.account);
                            let name = create_memo(move || member.with(|member| member.name.clone()));
                            let detail = create_memo(move || member.with(member_detail));
                            let managed = create_memo(move || member.with(|member| !member.me && !member.agent));
                            let promote = create_memo(move || member.with(|member| member.role != MemberRole::Administrator));
                            let role_label = create_memo(move || match promote.get() {
                                true => "Make administrator".to_owned(),
                                false => "Make editor".to_owned(),
                            });
                            let change = move || {
                                let role = match promote.get_untracked() {
                                    true => MemberRole::Administrator,
                                    false => MemberRole::Editor,
                                };
                                ask_core(CoreCommand::SetMemberRole(RoleChange { account, role }));
                            };
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <List spacing=Space::Xs2>
                                            <Text>{name}</Text>
                                            <Text style=TextStyle::CAPTION>{detail}</Text>
                                        </List>
                                    </ListItem>
                                    <Show value={managed}>
                                        <Button
                                            label={role_label}
                                            variant=ButtonVariant::Secondary
                                            disabled={busy}
                                            on_click={change}
                                        />
                                        <Button
                                            icon_only=true
                                            glyph=ICON_TRASH
                                            label="Remove"
                                            disabled={busy}
                                            on_click={move || removing.set(Some(account))}
                                        />
                                    </Show>
                                </List>
                            }
                        }</ForEach>
                    </DialogBody>
                </Show>
                <Show value={asks}>
                    <DialogBody
                        kind=DialogKind::Alert
                        title=REMOVE_MEMBER_TITLE
                        scrolls=false
                        actions={vec![
                            DialogAction::destructive("Remove", remove),
                            DialogAction::cancel("Keep"),
                        ]}
                        on_dismiss={move || removing.set(None)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{question}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// An agent's credentials as be-host's own dialog copies them: the JSON a program reads from
/// BE3_AGENT_CREDENTIALS.
fn agent_bundle(agent: &be_wasm_root_api::AgentCredentials) -> String {
    let quoted = |text: &str| {
        let mut quoted = String::from('"');
        for character in text.chars() {
            match character {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                character if character < ' ' => {
                    quoted.push_str(&format!("\\u{:04x}", u32::from(character)))
                }
                character => quoted.push(character),
            }
        }
        quoted.push('"');
        quoted
    };
    let id = agent.organization;
    let hex: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
    let organization = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    format!(
        "{{\"organization\":{},\"recovery_phrase\":{},\"server\":{},\"token\":{}}}",
        quoted(&organization),
        quoted(agent.phrase.as_str().unwrap_or_default()),
        quoted(&agent.server),
        quoted(agent.token.as_str().unwrap_or_default())
    )
}

/// The host's question before leaving a block an editor asked to leave: Leave, which asks the
/// host to and stays, busy, until the host no longer asks (it left, or why it could not shows),
/// or Stay (Escape, or closing it).
#[component]
fn LeaveDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let asked = create_memo(move || core.with(|view| view.leaving.clone()));
    let question = create_memo(move || {
        asked.with(|asked| match asked {
            Some(leaving) => match &leaving.within {
                Some(within) => format!(
                    "Leave {} in {within}? You lose access to it until someone shares it again.",
                    leaving.name
                ),
                None => format!(
                    "Leave {}? You lose access to it until someone shares it again.",
                    leaving.name
                ),
            },
            None => String::new(),
        })
    });
    let busy = create_memo(move || asked.with(|asked| asked.as_ref().is_some_and(|it| it.busy)));
    let refused = create_memo(move || asked.with(|asked| asked.as_ref()?.refused.clone()));
    let leave = move || {
        if let Some(block) = asked.with_untracked(|asked| asked.as_ref().map(|it| it.block)) {
            ask_core(CoreCommand::ConfirmLeave(block));
        }
    };
    escape_dismisses(done);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                kind=DialogKind::Alert
                title=LEAVE_TITLE
                scrolls=false
                actions={vec![
                    DialogAction::destructive("Leave", leave).busy(busy),
                    DialogAction::cancel("Stay"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{question}</Text>
                <be_host_accounts_ui::ErrorText text={refused} />
            </DialogBody>
        </Frame>
    }
}

/// A line saying how something went, while there is one: unlike an error, in the text's own
/// colour.
#[component]
fn Note(text: Memo<Option<String>>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Show value={text}>|text: ReadSignal<String>|
                <Text style=TextStyle::PARAGRAPH>{text}</Text>
            </Show>
        </List>
    }
}

/// Who a card names, as be-host words it: their name and email, or what they gave.
fn card_named(card: &ContactCard) -> String {
    match (card.name.trim().is_empty(), card.email.is_empty()) {
        (true, _) => card.email.clone(),
        (false, true) => card.name.clone(),
        (false, false) => format!("{} ({})", card.name, card.email),
    }
}

/// The person's contacts: each with Remove, which asks first in the list's place; a contact
/// link to make, which comes back once as `make-secret`'s answer and shows with Copy and Done;
/// and a pasted contact link to open, handed over as a secret and cleared, then who it is from,
/// to connect with or not. While a contact link is being made it stays open, as the agents do.
#[component]
fn ContactsDialog(
    core: Signal<CoreView>,
    link: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
    done: Callback<Closed>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.contacts.clone()));
    let contacts = create_memo(move || area.with(|area| area.contacts.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let offer = create_memo(move || {
        area.with(|area| {
            area.offered
                .as_ref()
                .map(|card| format!("Connect with {}?", card_named(card)))
        })
    });
    let offered = create_memo(move || offer.with(Option::is_some));
    let offer = create_memo(move || offer.get().unwrap_or_default());
    let connected = create_memo(move || {
        area.with(|area| {
            area.connected
                .as_ref()
                .map(|card| format!("You are connected with {}.", card_named(card)))
        })
    });
    let showing =
        create_memo(move || link.with(|link| link.as_ref().and_then(Held::link).is_some()));
    let removing = create_signal(None::<[u8; 16]>);
    let asked = create_memo(move || {
        let account = removing.get()?;
        contacts.with(|contacts| contacts.iter().find(|it| it.account == account).cloned())
    });
    let asks = create_memo(move || asked.with(Option::is_some));
    let listing = create_memo(move || !showing.get() && !asks.get());
    let question = create_memo(move || {
        asked.with(|asked| {
            let name = asked
                .as_ref()
                .map(|contact| {
                    card_named(&ContactCard {
                        name: contact.name.clone(),
                        email: contact.email.clone(),
                    })
                })
                .unwrap_or_default();
            format!("Remove {name}? Whatever you and they have shared with each other is unshared.")
        })
    });
    let shown = create_memo(move || {
        link.with(|link| {
            link.as_ref()
                .and_then(Held::link)
                .unwrap_or_default()
                .to_owned()
        })
    });
    let copy = move || {
        let text = shown.get_untracked();
        if !text.is_empty() {
            be_wasm_ops::guest::spawn(async move {
                let item = be_lib_ui_types::ClipItem::text(text);
                let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
            });
        }
    };
    let pasted = create_signal(String::new());
    let open_link = move || {
        // The link is a bearer secret: shown as typed, it leaves the field as it is handed over.
        let typed = pasted.get_untracked();
        pasted.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::OpenContactLink(Password::from(typed)));
        }
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            // While a contact link is being made it stays open, so the link reaches the
            // person rather than live and unheld.
            if closes && !pending.get_untracked() {
                match asks.get_untracked() {
                    true => removing.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=CONTACTS_TITLE
                        actions={vec![
                            DialogAction::cancel("Close").disabled(move || pending.get()),
                            DialogAction::primary("New link", move || ask.call(SecretCommand::ContactLink))
                                .busy(busy)
                                .disabled(move || pending.get()),
                        ]}
                        on_dismiss={move || if !pending.get_untracked() { done.call(Closed::Dismissed) }}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <Note text={connected} />
                        <Show value={offered}>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Sm
                            >
                                <ListItem grow=1.0>
                                    <Text>{offer}</Text>
                                </ListItem>
                                <Button
                                    label="Not now"
                                    variant=ButtonVariant::Secondary
                                    on_click={|| ask_core(CoreCommand::ForgetContactLink)}
                                />
                                <Button
                                    label="Connect"
                                    variant=ButtonVariant::Primary
                                    disabled={busy}
                                    on_click={|| ask_core(CoreCommand::AcceptContact)}
                                />
                            </List>
                        </Show>
                        <ForEach
                            value={contacts}
                            key={|contact: &ContactRow| contact.account}
                        >|contact: ReadSignal<ContactRow>| {
                            let account = contact.with_untracked(|contact| contact.account);
                            let name = create_memo(move || contact.with(|it| card_named(&ContactCard { name: it.name.clone(), email: it.email.clone() })));
                            let detail = create_memo(move || contact.with(contact_detail));
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <List spacing=Space::Xs2>
                                            <Text>{name}</Text>
                                            <Text style=TextStyle::CAPTION>{detail}</Text>
                                        </List>
                                    </ListItem>
                                    <Button
                                        icon_only=true
                                        glyph=ICON_TRASH
                                        label="Remove"
                                        on_click={move || removing.set(Some(account))}
                                    />
                                </List>
                            }
                        }</ForEach>
                        <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                            <ListItem grow=1.0>
                                <TextInput
                                    value={pasted}
                                    label="Contact link"
                                    placeholder="Contact link"
                                    on_submit={move |_: String| open_link()}
                                />
                            </ListItem>
                            <Button label="Open" disabled={busy} on_click={open_link} />
                        </List>
                    </DialogBody>
                </Show>
                <Show value={showing}>
                    <DialogBody
                        title="Contact link"
                        actions={vec![
                            DialogAction::secondary("Done", move || link.set(None)),
                            DialogAction::primary("Copy", copy),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::CAPTION>
                            "Whoever has this link can connect with you."
                        </Text>
                        <Text>{shown}</Text>
                    </DialogBody>
                </Show>
                <Show value={asks}>
                    <DialogBody
                        kind=DialogKind::Alert
                        title=REMOVE_CONTACT_TITLE
                        scrolls=false
                        actions={vec![
                            DialogAction::destructive("Remove", move || {
                                if let Some(contact) = asked.get_untracked() {
                                    ask_core(CoreCommand::RemoveContact(contact.account));
                                }
                                removing.set(None);
                            }),
                            DialogAction::cancel("Keep"),
                        ]}
                        on_dismiss={move || removing.set(None)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{question}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// Whether a contact's key is held, and how the server's record of it differed, in words.
fn contact_detail(contact: &ContactRow) -> String {
    match (&contact.mismatch, contact.verified) {
        (Some(ContactMismatch::Older), _) => {
            "The server served an older record of this contact: this device keeps its own."
                .to_owned()
        }
        (Some(ContactMismatch::Unreadable), _) => {
            "The server's record of this contact can't be read here: this device keeps its own."
                .to_owned()
        }
        (None, true) => "Verified".to_owned(),
        (None, false) => "Not verified yet".to_owned(),
    }
}

/// What a share link opens, as be-host's own dialog asks it: the block, where, what it lets
/// the person do, and whether they join its organization as a guest.
fn share_offered(preview: &SharePreview) -> String {
    let can = match preview.access {
        ShareAccess::Edit => "view and edit it",
        ShareAccess::Join => "join it while someone hosts it",
        ShareAccess::View => "view it",
    };
    let named = share_named(preview);
    let organization = share_organization(preview);
    let guest = match preview.guest {
        true => format!(
            " You will join {organization} as a guest: you see only what links share with you there."
        ),
        false => String::new(),
    };
    format!("{named}, in {organization}, is shared with you: you can {can}.{guest}")
}

/// A shared block's name as one line, cut short, in quotes; its maker's words.
fn share_named(preview: &SharePreview) -> String {
    let name: String = preview
        .name
        .split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match name.chars().count() {
        0 => "The block".to_owned(),
        count if count > 60 => format!("“{}…”", name.chars().take(60).collect::<String>()),
        _ => format!("“{name}”"),
    }
}

fn share_organization(preview: &SharePreview) -> String {
    match preview.organization.trim().is_empty() {
        true => "its organization".to_owned(),
        false => preview.organization.clone(),
    }
}

/// Opening a share link someone sent: the pasted link, handed over as a secret and cleared;
/// then what it shares, to open or not; then what it opened.
#[component]
fn ShareLinkDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.share_links.clone()));
    let busy = create_memo(move || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let offer = create_memo(move || area.with(|area| area.offered.as_ref().map(share_offered)));
    let offered = create_memo(move || offer.with(Option::is_some));
    let opened = create_memo(move || {
        area.with(|area| {
            area.opened.as_ref().map(|preview| {
                format!(
                    "{} is open to you: you find it under Shared with me in {}.",
                    share_named(preview),
                    share_organization(preview)
                )
            })
        })
    });
    let pasting = create_memo(move || !offered.get());
    let offer = create_memo(move || offer.get().unwrap_or_default());
    let pasted = create_signal(String::new());
    let open_link = move || {
        let typed = pasted.get_untracked();
        pasted.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::OpenShareLink(Password::from(typed)));
        }
    };
    escape_dismisses(done);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={pasting}>
                    <DialogBody
                        title=SHARE_LINK_TITLE
                        actions={vec![
                            DialogAction::cancel("Close"),
                            DialogAction::primary("Open", open_link).busy(busy),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <Note text={opened} />
                        <TextInput
                            value={pasted}
                            label="Share link"
                            placeholder="Share link"
                            focused=true
                            on_submit={move |_: String| open_link()}
                        />
                    </DialogBody>
                </Show>
                <Show value={offered}>
                    <DialogBody
                        title=SHARE_LINK_TITLE
                        actions={vec![
                            DialogAction::secondary("Not now", || ask_core(CoreCommand::ForgetShareLink)),
                            DialogAction::primary("Open it", || ask_core(CoreCommand::AcceptShareLink))
                                .busy(busy),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{offer}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// A member's email and role, in words.
fn member_detail(member: &MemberRow) -> String {
    let role = match (member.agent, &member.role) {
        (true, _) => "agent",
        (false, MemberRole::Administrator) => "administrator",
        (false, MemberRole::Editor) => "editor",
        (false, MemberRole::Guest) => "guest",
    };
    let detail = match member.email.is_empty() {
        true => role.to_owned(),
        false => format!("{}, {role}", member.email),
    };
    match member.unsigned {
        true => format!("{detail}, added by the server"),
        false => detail,
    }
}

/// The organization's agents: each with Remove, and a name to make one by, whose credentials
/// come back once, as the answer to the request that made it (`ask`, `make-secret`): shown in
/// the list's place with Copy and Done, held only while they show.
#[component]
fn AgentsDialog(
    core: Signal<CoreView>,
    made: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
    done: Callback<Closed>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.agents.clone()));
    let agents = create_memo(move || area.with(|area| area.agents.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy || !area.listed));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let showing =
        create_memo(move || made.with(|made| made.as_ref().and_then(Held::agent).is_some()));
    let listing = create_memo(move || !showing.get());
    let name = create_signal(String::new());
    let create = move || {
        let named = name.get_untracked().trim().to_owned();
        if !named.is_empty() && !pending.get_untracked() {
            name.set(String::new());
            ask.call(SecretCommand::CreateAgent(named));
        }
    };
    // What it shows and copies of the credentials, as be-host's own dialog does: the bundle a
    // program reads from BE3_AGENT_CREDENTIALS, the one copy it keeps beside them.
    let heading = create_memo(move || {
        made.with(|made| {
            let name = made.as_ref().and_then(Held::agent).map(|agent| agent.name.clone());
            format!(
                "Copy these secret credentials for {}. They are shown only once. Set BE3_AGENT_CREDENTIALS to this value.",
                name.unwrap_or_default()
            )
        })
    });
    let credentials = create_memo(move || {
        made.with(|made| {
            made.as_ref()
                .and_then(Held::agent)
                .map(agent_bundle)
                .unwrap_or_default()
        })
    });
    let copy = move || {
        let text = credentials.get_untracked();
        if !text.is_empty() {
            be_wasm_ops::guest::spawn(async move {
                let item = be_lib_ui_types::ClipItem::text(text);
                let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
            });
        }
    };
    // While an agent is being made it stays open, so its credentials, which cannot be had
    // again, reach the person: Close, its close button and Escape wait for the answer.
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes && !pending.get_untracked() {
                done.call(Closed::Dismissed);
            }
            closes
        },
    );
    let dismiss = move || {
        if !pending.get_untracked() {
            done.call(Closed::Dismissed);
        }
    };
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=AGENTS_TITLE
                        actions={vec![
                            DialogAction::cancel("Close").disabled(move || pending.get()),
                            DialogAction::primary("Create agent", create)
                                .busy(busy)
                                .disabled(move || pending.get() || name.with(|name| name.trim().is_empty())),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <be_host_accounts_ui::ErrorText text={error} />
                        <ForEach
                            value={agents}
                            key={|agent: &AgentRow| agent.account}
                        >|agent: ReadSignal<AgentRow>| {
                            let account = agent.with_untracked(|agent| agent.account);
                            let label = create_memo(move || agent.with(|agent| agent.name.clone()));
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <Text>{label}</Text>
                                    </ListItem>
                                    <Button
                                        label="Remove"
                                        variant=ButtonVariant::Secondary
                                        disabled={busy}
                                        on_click={move || ask_core(CoreCommand::RemoveAgent(account))}
                                    />
                                </List>
                            }
                        }</ForEach>
                        <TextInput
                            value={name}
                            label="New agent"
                            placeholder="New agent"
                            focused=true
                            on_submit={move |_: String| create()}
                        />
                    </DialogBody>
                </Show>
                <Show value={showing}>
                    <DialogBody
                        title="Agent made"
                        actions={vec![
                            DialogAction::secondary("Done", move || made.set(None)),
                            DialogAction::primary("Copy", copy),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::CAPTION>{heading}</Text>
                        <Text>{credentials}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// How an invitation stands, who made it and when, in words.
fn invitation_title(row: &InvitationRow) -> String {
    let state = match row.state {
        InvitationState::Waiting => "Waiting",
        InvitationState::Opened => "Opened",
        InvitationState::Accepted => "Accepted",
        InvitationState::Expired => "Expired",
        InvitationState::Revoked => "Revoked",
    };
    let made = format!("{state} · made by {} on {}", row.inviter, date(row.created));
    match (&row.state, &row.accepter, row.ended) {
        (InvitationState::Accepted, Some(by), Some(on)) => {
            format!("{made} · by {by} on {}", date(on))
        }
        (InvitationState::Revoked, _, Some(on)) => format!("{made} · revoked {}", date(on)),
        _ => format!("{made} · expires {}", date(row.expires)),
    }
}

/// A day in UTC, from milliseconds since the epoch, as `2026-11-15`.
fn date(milliseconds: i64) -> String {
    let days = milliseconds.div_euclid(86_400_000);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02}")
}

/// Makes Escape close the component's dialog, as dismissed, once nothing inside it took the
/// key first.
fn escape_dismisses(done: Callback<Closed>) {
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                done.call(Closed::Dismissed);
            }
            closes
        },
    );
}

/// An unsent block's name, or what the root calls one with none.
fn edit_name(edit: &UnsentEdit) -> String {
    edit.name.clone().unwrap_or_else(|| "Untitled".to_owned())
}

/// Why a block's edits went unsent, and how many never became a commit, in words.
fn edit_detail(edit: &UnsentEdit) -> String {
    let why = match (&edit.aside, edit.offerable) {
        (Some(SetAside::Refused(refusal)), _) => {
            let why = match refusal {
                Refusal::OverQuota => "it would go over a limit",
                Refusal::Invalid => "it could not take it as it is",
                Refusal::Policy => "your organization does not allow it",
                Refusal::NotPermitted => "you may not make it there",
                Refusal::Unexplained => "it would not say why",
            };
            format!("The server refused it: {why}")
        }
        (Some(SetAside::KeyOutdated), _) => {
            "It was locked offline with a key someone who lost access may hold: discard it to go on editing".to_owned()
        }
        (Some(SetAside::Misplaced), _) => {
            "It was locked offline under a folder it is not really in: discard it to go on editing".to_owned()
        }
        (None, true) => "You can edit it again".to_owned(),
        (None, false) => "You cannot edit it now".to_owned(),
    };
    match edit.kept_edits {
        0 => why,
        1 => format!("{why}. 1 unsaved edit"),
        kept => format!("{why}. {kept} unsaved edits"),
    }
}

/// Asks its host's block stack to do `command`.
fn ask_core(command: CoreCommand) {
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::ask_core(command).await;
    });
}

/// The strip of text: `TEXT`, and `LATER_TEXT` from `LATER` on, or the `name` the dialog gave.
#[component]
fn Strip(name: Signal<Option<String>>) -> NodeId {
    let later = create_signal(false);
    let timer = create_timer(move || {
        later.set(true);
        None
    });
    timer.start(LATER);
    let shown = create_memo(move || match (name.get(), later.get()) {
        (Some(name), _) => name,
        (None, true) => LATER_TEXT.to_owned(),
        (None, false) => TEXT.to_owned(),
    });
    view! {
        <Frame align=Align2::START padding=Space::Xs>
            <Text size=TextSize::Sm>{shown}</Text>
        </Frame>
    }
}

impl frames::Guest for Root {
    fn frame(&self, request: FrameRequest) -> LocalFuture<FrameDone> {
        let root = self.clone();
        Box::pin(async move {
            let time = Duration::from_nanos(request.time);
            root.own(time, Duration::from_nanos(request.wall_clock))
                .await
                .expect("the root's host gives it fonts and a device to draw its own surface on");
            root.settle_dialog()
                .expect("the root draws its dialog with beui");
            root.settle_lock()
                .expect("the root draws its lock screen with beui");
            root.settle_onboarding()
                .expect("the root draws its accounts with beui");
            if request.time == RESIZES {
                if let Some(own) = &mut *root.own.borrow_mut() {
                    own.resize(RESIZED);
                }
                *root.unpresented.borrow_mut() = Some(OwnSurface::new(UNPRESENTED_SURFACE, SIZE));
            } else if let Some(text) = &mut *root.text.borrow_mut() {
                text.frame()
                    .expect("the root draws its strip of text with beui");
            }
            let own = root
                .own
                .borrow()
                .as_ref()
                .map(|own| own.layer(ROOT_LAYER, pos2(0.0, 0.0), 1.0))
                .expect("the root made its own surface");
            let text = root
                .text
                .borrow()
                .as_ref()
                .map(|text| text.layer(TEXT_LAYER, pos2(0.0, 0.0)))
                .expect("the root made its strip of text");
            if let Some(toasts) = &mut *root.toasts.borrow_mut() {
                toasts.frame().expect("the root draws its toasts with beui");
            }
            if let Some(dialog) = &mut *root.dialog.borrow_mut() {
                dialog
                    .surface
                    .frame()
                    .expect("the root draws its dialog with beui");
            }
            if let Some(statistics) = &mut *root.statistics.borrow_mut() {
                statistics
                    .frame()
                    .expect("the root draws its statistics with beui");
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
            let mut layers = vec![own.clone(), text];
            if request.time == CLOSES {
                if let Some(app) = root.app.borrow_mut().take() {
                    root.embedding.borrow_mut().remove(app.instance());
                }
            } else if request.time != RESIZES {
                let app = root.app(time);
                let deadline = pin!(host::deadline());
                let drawn = pin!(app.frame(time));
                let late = !first(drawn, deadline).await;
                layers.extend(app.layer(SCREEN, pos2(ORIGIN.0, ORIGIN.1)));
                if late {
                    let mut marker = own;
                    marker.id = LATE_LAYER;
                    marker.surface = Surface::Root(1);
                    layers.push(marker);
                }
            }
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
            // Its statistics panel while it is open, over the app: it scrolls, so it takes
            // presses, but never the focus from what the person works in.
            if let Some(statistics) = &*root.statistics.borrow() {
                let mut layer = statistics.layer(
                    STATISTICS_LAYER,
                    pos2(STATISTICS_ORIGIN.0, STATISTICS_ORIGIN.1),
                );
                layer.input.focuses = false;
                layers.push(layer);
            }
            // While the dialog is open, over the rest: its scrim, which takes no press itself but
            // captures every one that misses the dialog for the root's own layer, which closes
            // it, and never takes the focus; and the dialog over it.
            if let Some(dialog) = &*root.dialog.borrow()
                && let Some(scrim) = &*root.scrim.borrow()
            {
                let mut layer = scrim.layer(SCRIM_LAYER, pos2(0.0, 0.0), 1.0);
                layer.input = LayerInput {
                    hit: Area::Nowhere,
                    captures: true,
                    focuses: false,
                    placer: Some(ROOT_LAYER),
                    ..LayerInput::default()
                };
                layers.push(layer);
                layers.push(
                    dialog
                        .surface
                        .layer(DIALOG_LAYER, pos2(DIALOG_ORIGIN.0, DIALOG_ORIGIN.1)),
                );
            }
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
            let mut embedding = root.embedding.borrow_mut();
            let wants = embedding.wants();
            let drawn = embedding.drew(list);
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
                _ => Vec::new(),
            };
            // While locked, the root takes no key ahead of the lock screen. Otherwise Escape is
            // the root's while its dialog is open and the focus is outside it.
            let outside = root.dialog.borrow().is_some()
                && embedding.focused(None).map(|layer| layer.id) != Some(DIALOG_LAYER);
            embedding.intercept(match locked {
                true => vec![KeyChord::new(UNLOCKS_TWICE, Modifiers::NONE)],
                false => chords(outside),
            });
            drop(embedding);
            root.heard(&drawn.own);
            root.heard(&focus);
            let embedding = root.embedding.borrow();
            let requests = embedding.requests(None);
            // Its documents want a frame at once (one heard input or was woken) or later.
            let documents = [&root.text, &root.toasts]
                .into_iter()
                .filter_map(|surface| surface.borrow().as_ref().and_then(BeuiSurface::next));
            let dialog = root
                .dialog
                .borrow()
                .as_ref()
                .and_then(|dialog| dialog.surface.next());
            let statistics = root
                .statistics
                .borrow()
                .as_ref()
                .and_then(BeuiSurface::next);
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
                .chain(documents)
                .chain(dialog)
                .chain(statistics)
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
                    _ => None,
                }
            });
            FrameDone {
                layers: drawn.layers,
                next: next.map(|at| u64::try_from(at.as_nanos()).unwrap_or(u64::MAX)),
                owed: wants.owed,
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
        let own = self
            .embedding
            .borrow_mut()
            .input(window, &events)
            .unwrap_or_default();
        self.heard(&own);
        // What it asked of the dialog is done with the next frame, which this asks for.
        if self.asked.get().is_some() {
            self.waker.wake();
        }
        Box::pin(async {})
    }
}

impl paintings::Guest for Root {
    /// Its own surface's painting, all `ROOT_COLOR`, and what beui painted of its strip and,
    /// while one shows, its toasts. After a `RESIZES` frame the painting has the surface's new
    /// size while the host still shows the first fill at the old one: the root never presents
    /// it again, which is what that frame tests (the host keeps a surface's last frame until it
    /// is presented anew).
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
        let text = self.text.borrow().as_ref().and_then(painted);
        // Its toasts only while one shows, as its layer list shows them.
        let toasts = self
            .toasts
            .borrow()
            .as_ref()
            .filter(|toasts| toasts.overlay_area() != Area::Nowhere)
            .and_then(painted);
        // Its scrim and dialog only while the dialog is open, as its layer list shows them.
        let dialog = self
            .dialog
            .borrow()
            .as_ref()
            .and_then(|dialog| painted(&dialog.surface));
        let scrim = dialog.as_ref().and_then(|_| {
            let scrim = self.scrim.borrow();
            let scrim = scrim.as_ref()?;
            let (width, height) = scrim.size();
            let painting = Painting {
                frame: PaintFrame {
                    size: [width, height],
                    pixels_per_point: 1.0,
                    background: SCRIM_COLOR.map(|channel| (channel * 255.0).round() as u8),
                    primitives: Vec::new(),
                },
                textures: Default::default(),
            };
            Some((u64::from(scrim.id()), painting))
        });
        // Its statistics panel only while it is open, its accounts while they show, and its
        // lock screen while locked.
        let statistics = self.statistics.borrow().as_ref().and_then(painted);
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
        let painted = ScreenPaintings {
            screens: own
                .into_iter()
                .chain(text)
                .chain(accounts)
                .chain(statistics)
                .chain(scrim)
                .chain(dialog)
                .chain(toasts)
                .chain(lock)
                .collect(),
            overflows: Vec::new(),
        };
        Box::pin(async move { painted.encode() })
    }
}

/// Waits for whichever of `left` and `right` finishes first: answers whether `left` did.
async fn first<A: Future, B: Future>(mut left: Pin<&mut A>, mut right: Pin<&mut B>) -> bool {
    std::future::poll_fn(|context| {
        if left.as_mut().poll(context).is_ready() {
            return Poll::Ready(true);
        }
        match right.as_mut().poll(context).is_ready() {
            true => Poll::Ready(false),
            false => Poll::Pending,
        }
    })
    .await
}

fn opening() -> Opening {
    Opening {
        instance: EditorInstanceId(1),
        editor: EDITOR,
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        kind: OpeningKind::Block(OpenBlock {
            block_id: [1; 16],
            block_type: BLOCK_TYPE,
            view_block: None,
            editable: true,
            page_origin: String::new(),
            host_palette: false,
        }),
    }
}

fn screen() -> ScreenRequest {
    let (width, height) = SCREEN_SIZE;
    ScreenRequest {
        screen: SCREEN,
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: ViewVec2 {
                x: width as f32,
                y: height as f32,
            },
            visible: ViewPos2::ZERO,
            pixel_width: width * RASTER,
            pixel_height: height * RASTER,
            scale_factor: RASTER as f32,
            raster: RASTER as f32,
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
        fit: false,
        origin: None,
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
