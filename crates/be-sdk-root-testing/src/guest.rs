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
    CoreCommand, CoreView, DisplayQuestion, FrameDone, FrameRequest, HostStatistics, LockState,
    Notice, PowerAction, Refusal, SavingState, SetAside, StatisticRow, SyncState, UnsentEdit,
    guest::{
        exports::{frames, input, paintings},
        host,
    },
};
use beui::icons::{Glyph, ICON_LOG_OUT, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW, ICON_TRASH};
use beui::native::painting::{Frame as PaintFrame, Painting, ScreenPaintings};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Align2, Direction, KeyPhase, NodeId, Overflow, Pos2, Rect, TextAlign, vec2};

use crate::{
    APP, APP_ENTRY, CLOSES, DIALOG_LAYER, DIALOG_ORIGIN, DIALOG_SIZE, DIALOG_SURFACE, DIALOG_TITLE,
    DISCARD_UNSENT_TITLE, KEEP_DISPLAY_TITLE, LATE_LAYER, LATER, LATER_TEXT, LOCK_LAYER,
    LOCK_SURFACE, LOCKS_SESSION, OPENS_DIALOG, OPENS_STATISTICS, OPENS_UNSENT, ORIGIN, RASTER,
    RESIZED, RESIZES, ROOT_COLOR, ROOT_LAYER, ROOT_SURFACE, SCREEN, SCREEN_SIZE, SCRIM_COLOR,
    SCRIM_LAYER, SCRIM_SURFACE, SIZE, SLOW_SAVE, SLOW_SAVE_TITLE, SLOW_SAVE_TOAST, STATISTICS_AREA,
    STATISTICS_LAYER, STATISTICS_ORIGIN, STATISTICS_SURFACE, STATISTICS_TITLE, STRAY_PASSWORD,
    TEXT, TEXT_LAYER, TEXT_STRIP, TEXT_SURFACE, TOAST_AREA, TOAST_LAYER, TOAST_SURFACE,
    UNLOCKS_TWICE, UNPRESENTED_SURFACE, UNSENT_TITLE,
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
    /// The core's view its list of unsent edits shows, while that list is its open dialog.
    core_shown: Rc<Cell<Option<Signal<CoreView>>>>,
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
}

/// Which dialog to open.
enum Asking {
    Rename,
    KeepDisplay(DisplayQuestion),
    /// The list of the block stack's unsent edits.
    Unsent,
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
            core: Rc::new(RefCell::new(CoreView {
                generation: 0,
                unsent: Vec::new(),
            })),
            core_shown: Rc::default(),
            unpresented: Rc::default(),
            scrim: Rc::default(),
            dialog: Rc::default(),
            asked: Rc::default(),
            named: Rc::default(),
            statistics: Rc::default(),
            shown_statistics: Rc::default(),
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
    /// stops answering, and shows it in the list of unsent edits while that is open.
    async fn hear_core(self) {
        let mut seen = 0;
        while let Ok(view) = host::core(seen).await {
            seen = view.generation;
            *self.core.borrow_mut() = view.clone();
            if let (Some(dialog), Some(shown)) =
                (&mut *self.dialog.borrow_mut(), self.core_shown.get())
            {
                dialog.surface.enter(|| shown.set(view));
            }
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
                view! {
                    <Root>
                        <StatisticsPanel statistics={statistics} />
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
        let asked = self.asked.take();
        let open_keep = self.dialog.borrow().as_ref().and_then(|dialog| dialog.keep);
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
            None => {}
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
        let withdrawn = open_keep.is_some() && open_keep != keep_round;
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
            _ => None,
        };
        if closed.is_some() || asked == Some(Asked::Close) || withdrawn || opening.is_some() {
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
        self.core_shown.set(None);
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
            Asking::Rename | Asking::Unsent => None,
        };
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
fn StatisticsPanel(statistics: Signal<HostStatistics>) -> NodeId {
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
            // The lock screen, or else the dialog, drawn for the first time takes the focus.
            let focus = match (&mut *root.lock.borrow_mut(), &mut *root.dialog.borrow_mut()) {
                (Some(locked), _) if !locked.focused => {
                    locked.focused = true;
                    embedding.focus(None, Some(LOCK_LAYER))
                }
                (None, Some(dialog)) if !dialog.focused => {
                    dialog.focused = true;
                    embedding.focus(None, Some(DIALOG_LAYER))
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
            let next = wants
                .next
                .into_iter()
                .chain(documents)
                .chain(dialog)
                .chain(statistics)
                .chain(lock)
                .min();
            // The input method is where the focused surface edits: an app's screen, or the
            // field of its dialog.
            let ime = embedding.ime(None).or_else(|| {
                let focused = embedding.focused(None)?;
                let dialog = root.dialog.borrow();
                let lock = root.lock.borrow();
                match focused.id {
                    DIALOG_LAYER => dialog.as_ref()?.surface.ime(focused),
                    LOCK_LAYER => lock.as_ref()?.surface.ime(focused),
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
        // Its statistics panel only while it is open, and its lock screen while locked.
        let statistics = self.statistics.borrow().as_ref().and_then(painted);
        let lock = self
            .lock
            .borrow()
            .as_ref()
            .and_then(|locked| painted(&locked.surface));
        let painted = ScreenPaintings {
            screens: own
                .into_iter()
                .chain(text)
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
