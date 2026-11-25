use beui_view::types::icons::Glyph;
use std::time::Duration;

use beui_view::components::overlay::Modality;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::PointerPress;
use beui_view::types::{Dismiss, Placement};
use beui_view::types::{Pos2, Rect, Vec2};
use beui_view::measure::measure;
use beui_view::window::{Window, WindowAnchor, WindowHandle, opens_windows};

use super::{DockState, State, TabId};
use beui_dock::DockMode;
use beui_view::types::Extent;

const ATTENTION: Duration = Duration::from_millis(400);

#[derive(Clone, Copy)]
pub(crate) struct DialogParts {
    pub(crate) title: Memo<String>,
    pub(crate) icon: Memo<Option<Glyph>>,
    pub(crate) size: Memo<Vec2>,
    pub(crate) modal: Memo<bool>,
    pub(crate) dismiss_on_outside_press: Memo<bool>,
    pub(crate) on_close: Option<Callback<()>>,
    pub(crate) content: BuildFn<NodeId>,
}

pub struct DockDialogHandle {
    pub tab: TabId,
    pub title: Memo<String>,
    pub notice: Memo<Option<String>>,
    pub icon: Memo<Option<Glyph>>,
    pub size: Memo<Vec2>,
    pub modal: Memo<bool>,
    pub focused: Memo<bool>,
    pub attention: ReadSignal<bool>,
    pub detached: Memo<bool>,
    pub closable: bool,
    pub close: Callback<()>,
    pub grab: Callback<Pos2>,
    pub drag: Callback<Pos2>,
    pub card: NodeRef,
    pub header: NodeRef,
    pub body: NodeId,
}

/// What a dialog's sheet face gets, in the stacked and gathered modes, where the dock shows a
/// dialog as a sheet rather than a window.
pub struct DockDialogSheetHandle {
    pub tab: TabId,
    pub title: Memo<String>,
    /// The size its body asks for: a body too tall for a fitted sheet opens full height.
    pub size: Memo<Vec2>,
    pub open: Memo<bool>,
    /// Asks the dialog's owner to close it (its `on_close`); a dialog with none stays.
    pub close: Callback<()>,
    /// Builds what shows the body in the sheet: call it in the sheet's content.
    pub body: BuildFn<NodeId>,
}

impl super::State {
    pub(super) fn dialog_parent(&self, tab: TabId) -> Option<TabId> {
        self.dialogs.with(|dialogs| {
            dialogs
                .iter()
                .find(|(dialog, _)| *dialog == tab)
                .map(|(_, parent)| *parent)
        })
    }

    pub(super) fn is_dialog(&self, tab: TabId) -> bool {
        self.dialog_parent(tab).is_some()
    }

    pub(super) fn dialog_root(&self, tab: TabId) -> Option<TabId> {
        let mut tab = tab;
        let mut seen = vec![tab];
        while let Some(parent) = self.dialog_parent(tab) {
            if seen.contains(&parent) {
                return None;
            }
            seen.push(parent);
            tab = parent;
        }
        Some(tab)
    }

    pub(super) fn waiting(&self, tabs: &[TabId]) -> bool {
        self.dialogs.get().iter().any(|(dialog, _)| {
            self.dialog
                .call(*dialog)
                .is_some_and(|parts| parts.modal.get())
                && self
                    .dialog_root(*dialog)
                    .is_some_and(|root| tabs.contains(&root))
        })
    }

    pub(super) fn dialog_shown(&self, tab: TabId) -> bool {
        let Some(root) = self.dialog_root(tab) else {
            return false;
        };
        let mode = self.mode.get();
        self.state.with(|state| state.is_shown(root, mode))
    }

    fn modal_child(&self, holder: TabId) -> Option<TabId> {
        let dialogs = self.dialogs.get_untracked();
        dialogs
            .iter()
            .rev()
            .filter(|(_, parent)| *parent == holder)
            .map(|(dialog, _)| *dialog)
            .find(|dialog| {
                self.dialog
                    .call(*dialog)
                    .is_some_and(|parts| parts.modal.get_untracked())
            })
    }

    pub(super) fn settle_dialog_focus(&self) {
        let focused = self.state.with_untracked(DockState::focused_tab);
        let mut current = self
            .dialog_focus
            .get_untracked()
            .filter(|dialog| self.is_dialog(*dialog) && self.dialog_root(*dialog) == focused);
        let mut seen = Vec::new();
        while let Some(holder) = current.or(focused) {
            let Some(child) = self
                .modal_child(holder)
                .filter(|child| !seen.contains(child))
            else {
                break;
            };
            seen.push(child);
            current = Some(child);
        }
        self.dialog_focus.set(current);
    }

    pub(super) fn release_dialog_focus(&self) {
        if self.dialog_focus.get_untracked().is_some() {
            self.dialog_focus.set(None);
        }
        self.settle_dialog_focus();
    }

    pub(super) fn focus_dialog(&self, tab: TabId) {
        let Some(root) = self.dialog_root(tab) else {
            return;
        };
        self.edit(|state| {
            if state.focused_tab() != Some(root) {
                state.show(root);
            }
        });
        self.dialog_focus.set(Some(tab));
        self.settle_dialog_focus();
    }

    pub(super) fn call_attention(&self) {
        if let Some(dialog) = self.dialog_focus.get_untracked() {
            self.attention.update(|(count, target)| {
                *count += 1;
                *target = Some(dialog);
            });
        }
    }

    pub(super) fn focus_into_dialog(&self) {
        let Some(tab) = self.dialog_focus.get_untracked() else {
            return;
        };
        let Some(frame) = self
            .dialog_bodies
            .value()
            .borrow()
            .get(&tab)
            .and_then(NodeRef::try_get)
        else {
            return;
        };
        if frame.focus_is_within() || frame.focus_pending_within() {
            return;
        }
        if let Some(first) = frame.focusables_within().first().copied() {
            first.focus();
        }
    }

    /// Where the hosted dock window that holds `node` lies in its own points, when one does.
    fn window_area(&self, node: NodeId) -> Option<Rect> {
        let window = window_of(node);
        let frames: Vec<NodeRef> = self.windows.value().borrow().values().copied().collect();
        frames
            .into_iter()
            .filter_map(|frame| frame.try_get())
            .find(|frame| window_of(*frame) == window)
            .and_then(|frame| measure(frame).get_untracked())
            .map(|measured| Rect::from_min_size(Pos2::ZERO, measured.rect.size()))
    }

    fn anchor_ref(&self, parent: TabId) -> Option<NodeRef> {
        if self.is_dialog(parent) {
            return self.dialog_frames.value().borrow().get(&parent).copied();
        }
        let shown = self.state.with_untracked(|state| state.contains(parent));
        let built = self
            .panels
            .with_untracked(|panels| panels.get(&parent).copied());
        built.filter(|_| shown).map(|built| {
            let panel = NodeRef::new();
            panel.fill(built);
            panel
        })
    }
}

#[component]
pub(super) fn DockDialogs(dock: State) -> NodeId {
    let keys = dock.dialogs;
    let state = dock.state;
    let described = dock.dialog;
    create_effect(move || {
        state.track();
        for (dialog, _) in keys.get() {
            if let Some(parts) = described.call(dialog) {
                parts.modal.track();
            }
        }
        untrack(|| dock.settle_dialog_focus());
    });
    view! {
        <beui_view::primitives::List spacing=0.0>
            <beui_view::reactive::ForEach value={keys}>|(tab, parent): (TabId, TabId)|
                <DockDialogView dock tab parent />
            </beui_view::reactive::ForEach>
        </beui_view::primitives::List>
    }
}

#[component]
fn DockDialogView(dock: State, tab: TabId, parent: TabId) -> NodeId {
    let Some(parts) = dock.dialog.call(tab) else {
        return view! {
            <Frame />
        };
    };
    let DialogParts {
        title,
        icon,
        size,
        modal,
        dismiss_on_outside_press,
        on_close,
        content,
    } = parts;
    let frame = NodeRef::new();
    dock.dialog_frames.value().borrow_mut().insert(tab, frame);
    let inside = NodeRef::new();
    dock.dialog_bodies.value().borrow_mut().insert(tab, inside);
    let card = NodeRef::new();
    let header = NodeRef::new();
    let handles = dock.handles;
    on_cleanup(move || {
        dock.dialog_frames.try_with(|frames| {
            let mut frames = frames.borrow_mut();
            if frames.get(&tab).is_some_and(|held| *held == frame) {
                frames.remove(&tab);
                dock.dialog_bodies
                    .try_with(|bodies| bodies.borrow_mut().remove(&tab));
            }
        });
        if handles.is_alive() {
            handles.update(|handles| handles.retain(|_, (held, _)| *held != header));
        }
        if dock.dialog_focus.get_untracked() == Some(tab) {
            dock.dialog_focus.set(None);
        }
    });
    let shown = create_memo(move || dock.dialog_shown(tab));
    // What the dialog belongs to: its parent's panel, or the parent dialog's frame. A signal,
    // since the parent's panel can be built after the dialog.
    let owner = create_signal(None::<NodeId>);
    create_effect(move || {
        if !shown.get() {
            return;
        }
        dock.panels.with(|panels| panels.get(&parent).copied());
        let found = untrack(|| dock.anchor_ref(parent)).and_then(|node| node.try_get());
        if found.is_some() && untrack(|| owner.get()) != found {
            owner.set(found);
        }
    });
    let pointed = create_memo(move || {
        if !shown.get() {
            return None;
        }
        dock.tab_control(parent).anchor().filter(Rect::is_positive)
    });
    let blocking = create_memo(move || shown.get() && modal.get());
    let scrim_radius = create_memo(move || match dock.is_dialog(parent) {
        true => dock.dialog_radius,
        false => 0.0,
    });
    let focus = dock.dialog_focus;
    let focused = create_memo(move || focus.get() == Some(tab));
    let attention = create_signal(false);
    let settle = create_timer(move || {
        attention.set(false);
        None
    });
    let requests = dock.attention;
    create_effect(move || {
        let (_, target) = requests.get();
        if target == Some(tab) {
            untrack(|| {
                attention.set(true);
                settle.start(ATTENTION);
            });
        }
    });
    let scope = dock.open_scope();
    let body = scope.context().run(|| {
        let control = dock.tab_control(tab);
        provide_context(control);
        provide_context(Modality(create_memo(|| false)));
        content.call()
    });
    store(scope);
    let sheet = dock.dialog_sheet;
    let sheeted = create_memo(move || {
        sheet.is_some() && matches!(dock.mode.get(), DockMode::Stacked | DockMode::Compact)
    });
    // In a sheet the body takes the sheet's width.
    let width = create_memo(move || match sheeted.get() {
        true => Extent::fraction(1.0),
        false => Extent::px(size.get().x),
    });
    let height = create_memo(move || Extent::px(size.get().y));
    let body = view! {
        <Frame
            @node_ref=&inside
            @test_id={format!("dock.dialog.{}", tab.value())}
            width=width
            height=height
        >
            {body}
        </Frame>
    };
    let close = Callback::new(move |()| {
        if let Some(close) = &on_close {
            close.call(());
        }
    });
    let closable = on_close.is_some();
    // Where the person moved it, once they have: it stays there, hung from its owner.
    let moved = create_signal(None::<Pos2>);
    let detached = create_memo(move || moved.with(Option::is_some));
    let grabbed = StoredCell::new(None::<(Rect, Pos2)>);
    let viewport = use_viewport_size();
    let hosting = opens_windows();
    create_effect(move || {
        if !shown.get() {
            untrack(|| moved.set(None));
        }
    });
    let grab = Callback::new(move |at: Pos2| {
        let from = frame
            .try_get()
            .and_then(|frame| measure(frame).get_untracked())
            .map(|measured| measured.rect);
        grabbed.set(from.map(|from| (from, at)));
        dock.focus_dialog(tab);
    });
    // Dragged by its bar, it moves with the pointer and stays inside the window it shows in,
    // as far off its edges as a card keeps.
    let drag = Callback::new(move |at: Pos2| {
        let Some((from, start)) = grabbed.get() else {
            return;
        };
        let area = untrack(|| {
            hosting
                .get()
                .then(|| owner.get().and_then(|owner| dock.window_area(owner)))
                .flatten()
                .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, viewport.get()))
        });
        let area = area.shrink(f32::from(Placement::CARD.edge_margin()));
        let most = Pos2::new(
            (area.max.x - from.width()).max(area.min.x),
            (area.max.y - from.height()).max(area.min.y),
        );
        let to = from.min + (at - start);
        moved.set(Some(Pos2::new(
            to.x.clamp(area.min.x, most.x),
            to.y.clamp(area.min.y, most.y),
        )));
    });
    let chrome = dock.dialog_chrome;
    let notice = create_memo(move || dock.notice(tab));
    // The face is built each time the dialog shows; the tab's content, built once above, is
    // the dock's and shows in it through a portal.
    let face = move || {
        let body = view! {
            <Portal node={Some(body)} />
        };
        let face = chrome.call(DockDialogHandle {
            tab,
            title,
            notice,
            icon,
            size,
            modal,
            focused,
            attention: attention.read_only(),
            detached,
            closable,
            close,
            grab,
            drag,
            card,
            header,
            body,
        });
        handles.update(|handles| {
            handles.insert(tab, (header, false));
        });
        face
    };
    let call_attention = move || {
        dock.focus_dialog(tab);
        dock.focus_into_dialog();
        dock.call_attention();
    };
    let overlay = NodeRef::new();
    let hosted = opens_windows();
    let shown_in = create_memo(move || {
        hosted
            .get()
            .then(|| owner.get().map(window_of))
            .flatten()
    });
    let windowed = create_memo(move || shown.get() && !sheeted.get());
    let anchor = create_memo(move || match (moved.get(), pointed.get()) {
        (Some(_), _) => None,
        (None, Some(rect)) => Some(WindowAnchor::Beside(rect)),
        (None, None) => Some(WindowAnchor::Top),
    });
    let at = create_memo(move || moved.get().unwrap_or(Pos2::ZERO));
    let view = view! {
        <Window
            @node_ref=&overlay
            size=Vec2::ZERO
            own_window=false
            owner={owner}
            anchor={anchor}
            at={at}
            blocks={blocking}
            scrim={dock.dialog_scrim}
            scrim_radius={scrim_radius}
            pointer={card}
            beak={dock.dialog_beak}
            shown_in={shown_in}
            open={windowed}
            on_dismiss={move |reason: Dismiss| match reason {
                Dismiss::OutsidePress if dismiss_on_outside_press.get_untracked() => close.call(()),
                Dismiss::OutsidePress if blocking.get_untracked() => call_attention(),
                Dismiss::Escape | Dismiss::Back => close.call(()),
                Dismiss::OutsidePress | Dismiss::WithParent => {}
            }}
        >|_: WindowHandle|
            <Interactive @node_ref=&frame on_press={move |_: PointerPress| dock.focus_dialog(tab)}>
                {face()}
            </Interactive>
        </Window>
    };
    // In the stacked and gathered modes the same body shows in a sheet.
    let view = match sheet {
        None => view,
        Some(sheet) => {
            let sheet = sheet.call(DockDialogSheetHandle {
                tab,
                title,
                size,
                open: create_memo(move || shown.get() && sheeted.get()),
                close,
                body: BuildFn::new(move || {
                    view! {
                        <Interactive
                            @node_ref=&frame
                            on_press={move |_: PointerPress| dock.focus_dialog(tab)}
                        >
                            <Portal node={Some(body)} />
                        </Interactive>
                    }
                }),
            });
            view! {
                <List spacing=0.0>
                    {view}
                    {sheet}
                </List>
            }
        }
    };
    dock.dialog_layers.value().borrow_mut().insert(tab, overlay);
    on_cleanup(move || {
        if let Some(layers) = dock.dialog_layers.try_value() {
            layers.borrow_mut().remove(&tab);
        }
        if let Some(overlay) = overlay.try_get() {
            overlay.close_overlay();
        }
    });
    create_effect(move || {
        if !(blocking.get() || focused.get()) {
            return;
        }
        if let Some(overlay) = overlay.try_get() {
            overlay.raise_overlay();
        }
    });
    create_effect(move || {
        if focused.get() {
            untrack(|| dock.focus_into_dialog());
        }
    });
    let was_shown = std::cell::Cell::new(false);
    create_effect(move || {
        let now = shown.get();
        if now && !was_shown.replace(true) {
            untrack(|| dock.focus_dialog(tab));
        }
    });
    view
}
