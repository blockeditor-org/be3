use beui_components_unstyled::DialogLayout;
use beui_core::icons::Glyph;
use std::time::Duration;

use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color;
use beui_core::geometry::{Rect, Vec2};
use beui_core::input::PointerPress;
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::NodeId;
use beui_core::window::{WindowPlacement, WindowReport};
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;
use beui_view::window::{Window, WindowHandle, opens_windows};

use super::{DockMode, DockState, DockTabControl, State, TabId};
use beui_view::types::Extent;

const ATTENTION: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DockDialogPresentation {
    #[default]
    Window,
    Card,
}

#[derive(Clone, Copy)]
pub(crate) struct DialogParts {
    pub(crate) presentation: DockDialogPresentation,
    pub(crate) title: Memo<String>,
    pub(crate) icon: Memo<Option<Glyph>>,
    pub(crate) size: Memo<Vec2>,
    pub(crate) modal: Memo<bool>,
    pub(crate) dismiss_on_outside_press: Memo<bool>,
    pub(crate) anchor: Memo<Option<Rect>>,
    pub(crate) on_close: Option<ClickCallback>,
    pub(crate) content: RenderFn<()>,
}

pub struct DockDialogHandle {
    pub tab: TabId,
    pub presentation: DockDialogPresentation,
    pub title: Memo<String>,
    pub notice: Memo<Option<String>>,
    pub icon: Memo<Option<Glyph>>,
    pub size: Memo<Vec2>,
    pub modal: Memo<bool>,
    pub focused: Memo<bool>,
    pub attention: ReadSignal<bool>,
    pub closable: bool,
    pub close: ClickCallback,
    pub pointer: NodeRef,
    pub body: NodeId,
}

pub struct DockSheetHandle {
    pub tab: TabId,
    pub open: Memo<bool>,
    pub close: ClickCallback,
    pub body: NodeId,
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
        let stacked = self.mode.get() == super::DockMode::Stacked;
        self.state.with(|state| state.is_shown(root, stacked))
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
        if current != self.dialog_focus.get_untracked() {
            self.dialog_focus.set(current);
        }
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

    fn anchor_ref(&self, parent: TabId) -> Option<NodeRef> {
        if self.is_dialog(parent) {
            return self.dialog_frames.value().borrow().get(&parent).copied();
        }
        let shown = self.state.with_untracked(|state| state.contains(parent));
        shown.then(|| {
            let panel = NodeRef::new();
            panel.fill(self.panel(parent));
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
        let _ = state.with(DockState::focused_tab);
        for (dialog, _) in keys.get() {
            if let Some(parts) = described.call(dialog) {
                parts.modal.get();
            }
        }
        untrack(|| dock.settle_dialog_focus());
    });
    view! {
        <beui_view::primitives::List spacing=0.0>
            <beui_view::reactive::ForEach keys>|(tab, parent): (TabId, TabId)|
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
        presentation,
        title,
        icon,
        size,
        modal,
        dismiss_on_outside_press,
        anchor: asked_from,
        on_close,
        content,
    } = parts;
    let card = presentation == DockDialogPresentation::Card;
    let frame = NodeRef::new();
    dock.dialog_frames.value().borrow_mut().insert(tab, frame);
    let inside = NodeRef::new();
    dock.dialog_bodies.value().borrow_mut().insert(tab, inside);
    on_cleanup(move || {
        dock.dialog_frames.try_with(|frames| {
            let mut frames = frames.borrow_mut();
            if frames.get(&tab).is_some_and(|held| *held == frame) {
                frames.remove(&tab);
                dock.dialog_bodies
                    .try_with(|bodies| bodies.borrow_mut().remove(&tab));
            }
        });
        if dock.dialog_focus.get_untracked() == Some(tab) {
            dock.dialog_focus.set(None);
        }
    });
    let shown = create_memo(move || dock.dialog_shown(tab));
    let mode = dock.mode;
    let fallback = dock.dialog_sheet;
    let stacked =
        create_memo(move || card && fallback.is_some() && mode.get() == DockMode::Stacked);
    let anchored = create_memo(move || shown.get() && !stacked.get());
    let in_sheet = create_memo(move || shown.get() && stacked.get());
    let anchor = create_signal(NodeRef::new());
    create_effect(move || {
        if !shown.get() {
            return;
        }
        if let Some(node) = untrack(|| dock.anchor_ref(parent)) {
            anchor.set(node);
        }
    });
    let blocking = create_memo(move || shown.get() && modal.get());
    let policy = create_memo(move || match blocking.get() {
        true => LayerPolicy::BlocksParent,
        false => LayerPolicy::Pass,
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
    let control = DockTabControl {
        tab,
        state: dock.state,
        dock,
    };
    let body = scope.context().run(|| {
        provide_context(control);
        provide_context(DialogLayout {
            narrow: stacked,
            sheet: stacked,
            bottom_inset: create_memo(|| 0.0),
        });
        content.call(())
    });
    store(scope);
    let width = create_memo(move || match (card, stacked.get()) {
        (true, true) => Extent::fraction(1.0),
        (true, false) => Extent::at_most(size.get().x),
        (false, _) => Extent::px(size.get().x),
    });
    let height = create_memo(move || match card {
        true => Extent::at_most(size.get().y),
        false => Extent::px(size.get().y),
    });
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
    let close = ClickCallback::new(move || {
        if let Some(close) = &on_close {
            close.call();
        }
    });
    let held = create_memo(move || (!stacked.get()).then_some(body));
    let pointer = NodeRef::new();
    let closable = on_close.is_some();
    let chrome = dock.dialog_chrome;
    let notice = create_memo(move || dock.notice(tab));
    let face = Render::new(move |()| {
        chrome.call(DockDialogHandle {
            tab,
            presentation,
            title,
            notice,
            icon,
            size,
            modal,
            focused,
            attention: attention.read_only(),
            closable,
            close,
            pointer,
            body: view! {
                <Portal node={held} />
            },
        })
    })
    .once();
    let lent = create_memo(move || stacked.get().then_some(body));
    let fallback = fallback.map(|render| {
        render.call(DockSheetHandle {
            tab,
            open: in_sheet,
            close,
            body: view! {
                <Portal node={lent} />
            },
        })
    });
    let call_attention = move || {
        dock.focus_dialog(tab);
        dock.focus_into_dialog();
        dock.call_attention();
    };
    let (margin, gap, dims) = (dock.dialog_margin, dock.dialog_gap, dock.dialog_scrim);
    let placed = create_memo(move || {
        let anchor = anchor.get();
        match (card, asked_from.get()) {
            (true, Some(rect)) => (
                OverlayAnchor::Pinned { node: anchor, rect },
                Placement::Popover { gap, margin },
                Color::TRANSPARENT,
            ),
            (true, None) => (
                OverlayAnchor::Node(anchor),
                Placement::HangFrom(margin),
                dims.get(),
            ),
            (false, _) => (
                OverlayAnchor::Node(anchor),
                Placement::CenterOn,
                Color::TRANSPARENT,
            ),
        }
    });
    let at = create_memo(move || placed.get().0);
    let placement = create_memo(move || placed.get().1);
    let scrim = create_memo(move || placed.get().2);
    let overlay = NodeRef::new();
    let hosted = opens_windows();
    let windowed = create_memo(move || !card && hosted.get());
    let face = view! {
        <Interactive @node_ref=&frame on_press={move |_: PointerPress| dock.focus_dialog(tab)}>
            {face.call(())}
        </Interactive>
    };
    let overlaid = create_memo(move || (!windowed.get()).then_some(face));
    let in_window = create_memo(move || windowed.get().then_some(face));
    let floating = create_memo(move || anchored.get() && !windowed.get());
    let windowed_open = create_memo(move || anchored.get() && windowed.get());
    let outset = dock.dialog_outset;
    let window_size = create_memo(move || size.get() + outset);
    let window_placement = create_memo(move || {
        windowed_open.get();
        let size = window_size.get();
        let node = anchor.get().try_get();
        untrack(|| {
            let node = node?;
            let rect = measure(node).get_untracked()?.rect;
            Some(WindowPlacement::Over {
                window: window_of(node),
                at: rect.center() - size / 2.0,
            })
        })
        .unwrap_or(WindowPlacement::AtPointer)
    });
    let window_title = create_memo(move || Some(title.get()));
    let window_notice = create_memo(move || dock.notice(tab));
    let view = view! {
        <List spacing=0.0>
            <Overlay
                @node_ref=&overlay
                anchor={at}
                placement
                pointer
                beak={dock.dialog_beak}
                scrim
                policy
                traps_focus=false
                open={floating}
                keep_alive=true
                on_dismiss={move |reason: Dismiss| match reason {
                    Dismiss::OutsidePress if dismiss_on_outside_press.get_untracked() => {
                        close.call()
                    }
                    Dismiss::OutsidePress if blocking.get_untracked() => call_attention(),
                    Dismiss::Escape | Dismiss::Back => close.call(),
                    Dismiss::OutsidePress | Dismiss::WithParent => {}
                }}
            >
                <Portal node={overlaid} />
            </Overlay>
            <Window
                id={None}
                size={window_size}
                placement={window_placement}
                title={window_title}
                notice={window_notice}
                open={windowed_open}
                keep_alive=true
                on_report={move |report: WindowReport| match report {
                    WindowReport::Closed(_) => close.call(),
                    WindowReport::Focused { focused: true, .. } => dock.focus_dialog(tab),
                    _ => {}
                }}
            >|_: WindowHandle|
                <Portal node={in_window} />
            </Window>
            {fallback}
        </List>
    };
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
