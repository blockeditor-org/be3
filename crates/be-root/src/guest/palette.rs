//! The one command palette the root draws over every app, as be-host draws its own
//! (`open-block.host-palette`): Ctrl+Shift+P, or an editor asking for it
//! (`shell-message.open-palette`), lists the live actions of the editor whose view has the
//! keyboard and of each editor placing it, outward, each under its app's name, innermost first;
//! choosing one gives that view the keyboard back and runs it (`app.run-action`)
//! (plans/host_as_platform_adapter.md, 4g7d1).

use be_lib_ui_types::KeyChord;
use be_sdk_api::{EditorInstanceId, PaletteMode, ScreenId};
use be_sdk_root::Embedded;

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

/// One row: what it shows, and the action it runs on the screen of the app that described it.
struct Row {
    item: PaletteItem,
    app: u64,
    screen: ScreenId,
    id: String,
}

/// The palette while it is open.
pub(super) struct Palette {
    surface: BeuiSurface,
    rows: Rc<Vec<Row>>,
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
        let from = self.embedding.borrow().focused_editor(None);
        self.open_palette(from.map(|(app, editor, screen)| (app, editor, Some(screen))));
    }

    /// Opens the palette an editor asked for (`shell-message.open-palette`), listing outward
    /// from that editor: its actions, then those of each editor placing it. Quick open (a
    /// palette of blocks) is not drawn under the root yet.
    pub(super) fn ask_palette(&self) {
        let asked = self.embedding.borrow().take_palette_ask();
        let Some((app, editor, mode)) = asked else {
            return;
        };
        if mode != PaletteMode::Actions || self.palette.borrow().is_some() {
            return;
        }
        self.open_palette(Some((app, editor, None)));
    }

    /// Opens the palette listing outward from `from`, the editor and the screen of it that has
    /// the keyboard, if one has.
    fn open_palette(&self, from: Option<(Embedded, EditorInstanceId, Option<ScreenId>)>) {
        let Some(beui) = self.beui.borrow().clone() else {
            return;
        };
        let rows = Rc::new(self.palette_rows(from));
        let items: Vec<PaletteItem> = rows.iter().map(|row| row.item.clone()).collect();
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
                let items = create_memo(move || items.clone());
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
        let Some((app, editor, screen)) = from else {
            return Vec::new();
        };
        let around = match &*self.session.borrow() {
            Some(session) => session.editors_around(app.instance(), editor),
            None => vec![(app.instance(), editor, String::new())],
        };
        let embedding = self.embedding.borrow();
        let mut rows = Vec::new();
        for (handle, shown, name) in around {
            let Some(app) = embedding.app_of(handle) else {
                continue;
            };
            let first = screen.filter(|_| shown == editor);
            let mut actions: Vec<_> = app
                .actions_of(shown, first)
                .into_iter()
                .filter(|(_, action)| action.live)
                .collect();
            actions.sort_by_key(|(_, action)| !action.enabled);
            for (screen, action) in actions {
                rows.push(Row {
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
                    app: handle,
                    screen,
                    id: action.id,
                });
            }
        }
        rows
    }

    /// Closes the palette once its document said it closed or a row was picked: it gives the
    /// keyboard back to the layer that had it, then runs the row picked on its app's screen.
    pub(super) fn settle_palette(&self) {
        let picked = {
            let palette = self.palette.borrow();
            let Some(palette) = palette.as_ref() else {
                return;
            };
            let picked = palette.chosen.borrow_mut().take();
            match (picked, palette.closed.get()) {
                (Some(key), _) => Some(
                    palette
                        .rows
                        .iter()
                        .find(|row| row.item.key == key)
                        .map(|row| (row.app, row.screen, row.id.clone())),
                ),
                (None, true) => Some(None),
                (None, false) => None,
            }
        };
        let Some(run) = picked else {
            return;
        };
        let closed = self.palette.borrow_mut().take();
        if let Some(palette) = closed {
            self.close_palette(palette);
        }
        if let Some((app, screen, id)) = run
            && let Some(app) = self.embedding.borrow().app_of(app)
        {
            app.run_action(screen, id);
        }
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
