//! The one command palette the root draws over every app, as be-host draws its own
//! (`open-block.host-palette`): Ctrl+Shift+P, or an editor asking for it
//! (`shell-message.open-palette`), lists the live actions of the editor whose view has the
//! keyboard and of each editor placing it, outward, each under its app's name, innermost first;
//! choosing one gives that view the keyboard back and runs it (`app.run-action`). An editor
//! asking for quick open (`PaletteMode::Blocks`, the workspace's Ctrl+P) gets it as a palette of
//! blocks, which the host finds for what is typed (`host.find-blocks`) and opens the one chosen
//! as that editor asked to (`host.open-block`) (plans/host_as_platform_adapter.md, 4g7d1).

use be_lib_ui_types::KeyChord;
use be_sdk_api::{EditorInstanceId, PaletteMode, ScreenId};
use be_sdk_root::Embedded;
use be_wasm_root_api::{OpenableBlock, OpenedIn};

use super::*;
use crate::PALETTE_SURFACE;

/// The chord that opens the palette, and closes it while it is open, as be-host's does.
pub(super) fn palette_chord() -> KeyChord {
    KeyChord::new(
        Key::P,
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
    )
}

/// One row: what it shows, and what choosing it does.
#[derive(Clone)]
struct Row {
    item: PaletteItem,
    run: Run,
    /// The block of the editor whose action it runs, which names it once the host does.
    block: Option<[u8; 16]>,
}

/// What choosing a row does.
#[derive(Clone)]
enum Run {
    /// Runs the action `id` on the screen of the app `app` that described it.
    Action {
        app: u64,
        screen: ScreenId,
        id: String,
    },
    /// Opens the block (quick open).
    Open(OpenableBlock),
    /// Asks the host to do one of its own (`host.ask-core`), the command this makes.
    Core(fn() -> CoreCommand),
}

/// Quick open while the palette shows it: what is typed, the rows it shows, the editor it was
/// asked from, and the host's answers.
struct Blocks {
    /// What is typed, as it changed since the root last asked the host for it, which the
    /// palette document's search field sets.
    typed: Rc<RefCell<Option<String>>>,
    /// The editor that asked for it, which the block chosen opens as.
    from: Option<OpenedIn>,
    /// The host's answer to the find under way, once it answered, until shown.
    found: Rc<RefCell<Option<Vec<OpenableBlock>>>>,
    /// The find under way (`host.find-blocks`), which stops, and with it the host's search, as
    /// what is typed changes or the palette closes.
    finding: Option<Rc<Stop>>,
}

impl Drop for Blocks {
    fn drop(&mut self) {
        if let Some(finding) = self.finding.take() {
            finding.stop();
        }
    }
}

/// Stops a find under way: its call to the host, dropped, is cancelled.
#[derive(Default)]
struct Stop {
    stopped: Cell<bool>,
    waker: RefCell<Option<std::task::Waker>>,
}

impl Stop {
    fn stop(&self) {
        self.stopped.set(true);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

/// The palette while it is open.
pub(super) struct Palette {
    surface: BeuiSurface,
    rows: Rc<Vec<Row>>,
    /// The rows shown, the palette document's own, once it ran.
    items: Rc<Cell<Option<Signal<Vec<PaletteItem>>>>>,
    /// The names of the blocks of the editors it lists, once the host named them
    /// (`host.block-names`), until its rows are named after them.
    names: Rc<RefCell<Option<Vec<([u8; 16], String)>>>>,
    /// Quick open, while it shows blocks.
    blocks: Option<Blocks>,
    /// The row picked, by its key, which its document sets.
    chosen: Rc<RefCell<Option<String>>>,
    /// Whether it closed with nothing picked (Escape, a press outside), which its document sets.
    closed: Rc<Cell<bool>>,
    /// The layer that had the keyboard as it opened, which gets it back once it closes.
    had: Option<LayerId>,
    /// Whether it took the keyboard yet, once drawn.
    focused: bool,
}

impl Root {
    /// Opens the palette over every app, or closes it while it is open, as Ctrl+Shift+P does:
    /// listing outward from the editor whose view has the keyboard.
    pub(super) fn toggle_palette(&self) {
        let open = self.palette.borrow_mut().take();
        if let Some(palette) = open {
            self.close_palette(palette);
            return;
        }
        if self.palette_barred() {
            return;
        }
        let from = self.embedding.borrow().focused_editor(None);
        self.open_palette(
            from.map(|(app, editor, screen)| (app, editor, Some(screen))),
            PaletteMode::Actions,
        );
    }

    /// Opens the palette an editor asked for (`shell-message.open-palette`): its actions, then
    /// those of each editor placing it, or quick open, its blocks.
    pub(super) fn ask_palette(&self) {
        let asked = self.embedding.borrow().take_palette_ask();
        let Some((app, editor, mode)) = asked else {
            return;
        };
        if self.palette.borrow().is_some() || self.palette_barred() {
            return;
        }
        self.open_palette(Some((app, editor, None)), mode);
    }

    /// Opens the palette listing outward from `from`, the editor and the screen of it that has
    /// the keyboard, if one has: its actions, or for quick open the blocks the host finds, the
    /// block chosen opening as that editor asked to.
    fn open_palette(
        &self,
        from: Option<(Embedded, EditorInstanceId, Option<ScreenId>)>,
        mode: PaletteMode,
    ) {
        let Some(beui) = self.beui.borrow().clone() else {
            return;
        };
        let blocks = (mode == PaletteMode::Blocks).then(|| Blocks {
            // Every block, by name, for nothing typed yet.
            typed: Rc::new(RefCell::new(Some(String::new()))),
            from: from.as_ref().map(|(app, editor, _)| OpenedIn {
                instance: app.instance(),
                editor: editor.0,
            }),
            found: Rc::default(),
            finding: None,
        });
        let rows = Rc::new(match mode {
            PaletteMode::Actions => self.palette_rows(from),
            PaletteMode::Blocks => Vec::new(),
        });
        let items: Vec<PaletteItem> = rows.iter().map(|row| row.item.clone()).collect();
        let items_out: Rc<Cell<Option<Signal<Vec<PaletteItem>>>>> = Rc::default();
        let shown_out = Rc::clone(&items_out);
        let typed_out = blocks.as_ref().map(|blocks| Rc::clone(&blocks.typed));
        // The rows are named after the blocks of the editors they run on once the host names
        // them, as be-host's palette names them.
        let names: Rc<RefCell<Option<Vec<([u8; 16], String)>>>> = Rc::default();
        let mut named: Vec<[u8; 16]> = rows.iter().filter_map(|row| row.block).collect();
        named.sort_unstable();
        named.dedup();
        if !named.is_empty() {
            let asked = host::block_names(named.clone());
            let names = Rc::clone(&names);
            let waker = self.waker.clone();
            be_wasm_ops::guest::spawn(async move {
                // A host that stopped answering stops the root too.
                if let Ok(answer) = asked.await {
                    *names.borrow_mut() = Some(
                        named
                            .into_iter()
                            .zip(answer)
                            .filter_map(|(block, name)| Some((block, name?)))
                            .collect(),
                    );
                    waker.wake();
                }
            });
        }
        let (placeholder, label) = match mode {
            PaletteMode::Actions => ("Run a command", "Command palette"),
            PaletteMode::Blocks => ("Go to a file", "Quick open"),
        };
        let chosen: Rc<RefCell<Option<String>>> = Rc::default();
        let closed: Rc<Cell<bool>> = Rc::default();
        let (picks, closes) = (Rc::clone(&chosen), Rc::clone(&closed));
        let waker = self.waker.clone();
        let wakes = self.waker.clone();
        let window = self.window.get();
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(PALETTE_SURFACE, window.whole()),
            window.pixels_per_point,
            move || waker.wake(),
            move || {
                let open = create_signal(true);
                let query = create_signal(String::new());
                let shown = create_signal(items.clone());
                shown_out.set(Some(shown));
                if let Some(typed_out) = typed_out.clone() {
                    // Quick open asks the host again as what is typed changes.
                    let asks = wakes.clone();
                    create_effect_on(
                        move || query.get(),
                        move |typed, _| {
                            *typed_out.borrow_mut() = Some(typed.clone());
                            asks.wake();
                        },
                    );
                }
                let items = create_memo(move || shown.get());
                let wakes_too = wakes.clone();
                create_effect(move || {
                    if !open.get() {
                        closes.set(true);
                        wakes_too.wake();
                    }
                });
                view! {
                    <Root clear_color=ColorRole::None>
                        <CommandPalette
                            open
                            items={PaletteItems::list(items)}
                            score={PaletteItem::fuzzy_score as PaletteScore}
                            query
                            placeholder={placeholder.to_owned()}
                            label={label.to_owned()}
                            id="root.palette"
                            on_choose={move |key: String| {
                                *picks.borrow_mut() = Some(key);
                                wakes.wake();
                            }}
                        />
                    </Root>
                }
            },
        );
        let Ok(surface) = surface else {
            // The palette is a convenience: one that cannot be drawn stays closed.
            return;
        };
        let had = self.embedding.borrow().focused(None).map(|layer| layer.id);
        *self.palette.borrow_mut() = Some(Palette {
            surface,
            rows,
            items: items_out,
            names,
            blocks,
            chosen,
            closed,
            had,
            focused: false,
        });
    }

    /// The rows outward from `from`: the live actions of its editor (those of the screen that
    /// has the keyboard first), then those of each editor placing it, each under its app's
    /// name, the disabled ones after the rest of each editor's, as be-host's palette lists them.
    fn palette_rows(
        &self,
        from: Option<(Embedded, EditorInstanceId, Option<ScreenId>)>,
    ) -> Vec<Row> {
        let mut rows = self.editor_rows(from);
        rows.extend(self.host_rows());
        rows
    }

    /// The host's own rows, after every editor's, as be-host's palette lists its own: switching
    /// account while a session shows, and its theme.
    fn host_rows(&self) -> Vec<Row> {
        // A session shows once nothing is waited for and one is chosen (`settle_session`).
        let shown = {
            let core = self.core.borrow();
            core.waits_for.is_none() && core.showing.is_some()
        };
        let row = |id: &str, title: &str, glyph, enabled: bool, command: fn() -> CoreCommand| Row {
            item: PaletteItem {
                key: format!("host/{id}"),
                title: title.to_owned(),
                detail: "be-host".to_owned(),
                glyph: Some(glyph),
                disabled: !enabled,
                ..PaletteItem::default()
            },
            run: Run::Core(command),
            block: None,
        };
        let mut rows = vec![
            row(
                "host.switch-account",
                "Switch account",
                beui::icons::ICON_USERS,
                shown,
                || CoreCommand::SwitchAccount,
            ),
            row(
                "host.theme",
                "Switch between system, light and dark theme",
                beui::icons::ICON_SUN_MOON,
                true,
                || CoreCommand::CycleTheme,
            ),
        ];
        rows.sort_by_key(|row| row.item.disabled);
        rows
    }

    /// The rows of the editors outward from `from` (`palette_rows`).
    fn editor_rows(&self, from: Option<(Embedded, EditorInstanceId, Option<ScreenId>)>) -> Vec<Row> {
        let Some((app, editor, screen)) = from else {
            return Vec::new();
        };
        let around = match &*self.session.borrow() {
            Some(session) => session.editors_around(app.instance(), editor),
            None => vec![be_sdk_root::Around {
                app: app.instance(),
                editor,
                name: String::new(),
                block: None,
                placing: None,
            }],
        };
        let embedding = self.embedding.borrow();
        let mut rows = Vec::new();
        for be_sdk_root::Around {
            app: handle,
            editor: shown,
            name,
            block,
            placing,
        } in around
        {
            let Some(app) = embedding.app_of(handle) else {
                continue;
            };
            // The screen that has the keyboard first, and for each placer the one placing the
            // editor listed before, as be-host lists the placing view's first.
            let first = match placing {
                Some(view) => app.screen_of(shown, view),
                None => screen,
            };
            let mut actions: Vec<_> = app
                .actions_of(shown, first)
                .into_iter()
                .filter(|(_, action)| action.live)
                .collect();
            actions.sort_by_key(|(_, action)| !action.enabled);
            for (screen, action) in actions {
                rows.push(Row {
                    block,
                    run: Run::Action {
                        app: handle,
                        screen,
                        id: action.id.clone(),
                    },
                    item: PaletteItem {
                        key: format!("{handle}/{}/{}", screen.0, action.id),
                        title: action.label,
                        detail: name.clone(),
                        glyph: action.glyph.as_deref().and_then(beui::icons::named),
                        shortcut: action
                            .shortcut
                            .map(|chord| chord.label())
                            .unwrap_or_default(),
                        checked: action.checked,
                        disabled: !action.enabled,
                        ..PaletteItem::default()
                    },
                });
            }
        }
        rows
    }

    /// Closes the palette once its document said it closed or a row was picked: it gives the
    /// keyboard back to the layer that had it, or for a row picked to the layer showing the
    /// screen it runs on, as be-host focuses the chosen row's view, then runs it there.
    pub(super) fn settle_palette(&self) {
        self.find_blocks();
        self.name_rows();
        let picked = {
            let palette = self.palette.borrow();
            let Some(palette) = palette.as_ref() else {
                return;
            };
            let picked = palette.chosen.borrow_mut().take();
            match (picked, palette.closed.get()) {
                (Some(key), _) => Some(palette.rows.iter().find(|row| row.item.key == key).map(
                    |row| match &row.run {
                        Run::Action { app, screen, id } => Run::Action {
                            app: *app,
                            screen: *screen,
                            id: id.clone(),
                        },
                        Run::Open(block) => Run::Open(block.clone()),
                        Run::Core(command) => Run::Core(*command),
                    },
                )),
                (None, true) => Some(None),
                (None, false) => None,
            }
        };
        let Some(run) = picked else {
            return;
        };
        let closed = self.palette.borrow_mut().take();
        let from = closed
            .as_ref()
            .and_then(|palette| palette.blocks.as_ref())
            .and_then(|blocks| blocks.from);
        let action = match &run {
            Some(Run::Action { app, screen, id }) => self
                .embedding
                .borrow()
                .app_of(*app)
                .map(|app| (app, *screen, id.clone())),
            _ => None,
        };
        if let Some(mut palette) = closed {
            if let Some((app, screen, _)) = &action {
                palette.had = Some(app.layer_id(*screen));
            }
            self.close_palette(palette);
        }
        if let Some((app, screen, id)) = action {
            app.run_action(screen, id);
        }
        if let Some(Run::Core(command)) = &run {
            ask_core(command());
        }
        // The block chosen opens as the editor that asked for quick open asked to.
        if let Some(Run::Open(block)) = run {
            let opened = host::open_block(from, block);
            be_wasm_ops::guest::spawn(async move {
                // A host that stopped answering stops the root too.
                let _ = opened.await;
            });
        }
    }

    /// Names each editor's rows after its block, once the host named the blocks.
    fn name_rows(&self) {
        let mut palette = self.palette.borrow_mut();
        let Some(palette) = palette.as_mut() else {
            return;
        };
        // Until the palette's document ran, the names wait for it.
        let Some(items) = palette.items.get() else {
            return;
        };
        let Some(names) = palette.names.borrow_mut().take() else {
            return;
        };
        let rows: Vec<Row> = palette
            .rows
            .iter()
            .cloned()
            .map(|mut row| {
                if let Some((_, name)) = row
                    .block
                    .and_then(|block| names.iter().find(|(named, _)| *named == block))
                {
                    row.item.detail = name.clone();
                }
                row
            })
            .collect();
        let shown: Vec<PaletteItem> = rows.iter().map(|row| row.item.clone()).collect();
        palette.rows = Rc::new(rows);
        palette.surface.enter(|| items.set(shown));
    }

    /// Keeps quick open's rows to what the host finds for what is typed: asks it again as that
    /// changes (`host.find-blocks`), stopping the find under way, and shows the answer to the
    /// last, in the host's order.
    fn find_blocks(&self) {
        let mut palette = self.palette.borrow_mut();
        let Some(palette) = palette.as_mut() else {
            return;
        };
        let Some(blocks) = palette.blocks.as_mut() else {
            return;
        };
        let Some(items) = palette.items.get() else {
            return;
        };
        let typed = blocks.typed.borrow_mut().take();
        if let Some(typed) = typed {
            if let Some(finding) = blocks.finding.take() {
                finding.stop();
            }
            // An answer the last find gave that is not shown yet is for what was typed before.
            blocks.found.borrow_mut().take();
            let stop = Rc::new(Stop::default());
            blocks.finding = Some(Rc::clone(&stop));
            let finding = host::find_blocks(typed);
            let found = Rc::clone(&blocks.found);
            let waker = self.waker.clone();
            be_wasm_ops::guest::spawn(async move {
                let mut finding = std::pin::pin!(finding);
                let answer = std::future::poll_fn(|context| {
                    if stop.stopped.get() {
                        return std::task::Poll::Ready(None);
                    }
                    *stop.waker.borrow_mut() = Some(context.waker().clone());
                    finding.as_mut().poll(context).map(Some)
                })
                .await;
                // Stopped, the find is dropped, which cancels it; a host that stopped answering
                // stops the root too.
                if let Some(Ok(answer)) = answer {
                    *found.borrow_mut() = Some(answer);
                    waker.wake();
                }
            });
        }
        let Some(found) = blocks.found.borrow_mut().take() else {
            return;
        };
        let count = found.len();
        let rows: Vec<Row> = found
            .into_iter()
            .enumerate()
            .map(|(at, block)| Row {
                item: PaletteItem {
                    key: block
                        .block
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                    title: block.name.clone(),
                    detail: block.detail.clone(),
                    glyph: Some(beui::icons::ICON_FILE),
                    // In the host's order, which a search answers in.
                    ranked: Some(u32::try_from(count - at).unwrap_or(u32::MAX)),
                    ..PaletteItem::default()
                },
                run: Run::Open(block),
                block: None,
            })
            .collect();
        let shown: Vec<PaletteItem> = rows.iter().map(|row| row.item.clone()).collect();
        palette.rows = Rc::new(rows);
        palette.surface.enter(|| items.set(shown));
    }

    /// Whether the palette stays shut, as be-host's does: while the session is locked, and while
    /// the root's dialog or onboarding is up, each of which keeps the keyboard until it closes.
    fn palette_barred(&self) -> bool {
        self.lock.borrow().is_some()
            || self
                .lock_state
                .borrow()
                .as_ref()
                .is_some_and(|state| state.locked)
            || self.dialog.borrow().is_some()
            || self.onboarding.borrow().is_some()
    }

    /// Shuts the palette, if it is open, as the lock screen, a dialog or onboarding comes up
    /// over it: the layer that had the keyboard as it opened, which the palette answers, gets it
    /// back once what came up closes.
    pub(super) fn shut_palette(&self) -> Option<Option<LayerId>> {
        self.palette.borrow_mut().take().map(|palette| palette.had)
    }

    /// Lets go of `palette`, giving the keyboard back to the layer that had it as it opened.
    fn close_palette(&self, palette: Palette) {
        let told = self.embedding.borrow_mut().focus(None, palette.had);
        self.heard(&told);
    }

    /// Draws the palette's frame.
    pub(super) fn palette_frame(&self) {
        if let Some(palette) = &mut *self.palette.borrow_mut() {
            // The palette is a convenience: one that cannot be drawn shows nothing.
            let _ = palette.surface.frame();
        }
    }

    /// Takes the keyboard for the palette once it was drawn, the first time.
    pub(super) fn palette_focus(&self, embedding: &mut Embedding) -> Vec<Delivery> {
        let mut palette = self.palette.borrow_mut();
        match palette.as_mut() {
            Some(palette) if !palette.focused => {
                palette.focused = true;
                embedding.focus(None, Some(PALETTE_LAYER))
            }
            _ => Vec::new(),
        }
    }

    /// The layer showing the palette while it is open: over everything but the lock screen,
    /// filling the window, its scrim taking every press and the keyboard.
    pub(super) fn palette_layer(&self) -> Option<be_lib_layers::Layer> {
        let palette = self.palette.borrow();
        let mut layer = palette
            .as_ref()?
            .surface
            .layer(PALETTE_LAYER, pos2(0.0, 0.0));
        layer.input = LayerInput {
            hit: Area::Whole,
            captures: true,
            focuses: true,
            ..LayerInput::default()
        };
        Some(layer)
    }

    /// Lays the palette out over the whole window as it is now.
    pub(super) fn palette_resize(&self, whole: (u32, u32), raster: f32) {
        if let Some(palette) = &mut *self.palette.borrow_mut() {
            palette.surface.resize(whole, raster);
        }
    }

    /// Hands the palette what the root's router delivered to it.
    pub(super) fn palette_heard(&self, own: &[Delivery]) {
        if let Some(palette) = &mut *self.palette.borrow_mut() {
            palette.surface.input(own);
        }
    }

    /// When the palette wants its next frame.
    pub(super) fn palette_next(&self) -> Option<Duration> {
        self.palette.borrow().as_ref()?.surface.next()
    }

    /// Runs `each` on the palette's surface, while it is open.
    pub(super) fn palette_surface(&self, each: &mut dyn FnMut(&mut BeuiSurface)) {
        if let Some(palette) = &mut *self.palette.borrow_mut() {
            each(&mut palette.surface);
        }
    }

    /// What the palette painted last, while it is open.
    pub(super) fn palette_painted(&self) -> Option<(u64, Painting)> {
        let palette = self.palette.borrow();
        let surface = &palette.as_ref()?.surface;
        Some((u64::from(surface.id()), surface.painting()?))
    }

    /// The input method area of the palette's search field, while it has the keyboard.
    pub(super) fn palette_ime(
        &self,
        focused: &be_lib_layers::Layer,
    ) -> Option<be_lib_ui_types::ImeArea> {
        self.palette.borrow().as_ref()?.surface.ime(focused)
    }
}
