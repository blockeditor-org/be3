use std::time::Duration;

use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Vec2;
use beui_core::input::PointerPress;
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    ClickCallback, Frame, Interactive, Memo, NodeRef, ReadSignal, RenderFn, create_effect,
    create_memo, create_signal, create_timer, node_scope, on_cleanup, provide_context, untrack,
    with_document,
};

use super::{DockState, DockTabControl, State, TabId};

const ATTENTION: Duration = Duration::from_millis(400);

#[derive(Clone)]
pub(crate) struct DialogParts {
    pub(crate) title: Memo<String>,
    pub(crate) icon: Memo<String>,
    pub(crate) size: Memo<Vec2>,
    pub(crate) modal: Memo<bool>,
    pub(crate) on_close: Option<ClickCallback>,
    pub(crate) content: RenderFn<()>,
}

pub struct DockDialogHandle {
    pub tab: TabId,
    pub title: Memo<String>,
    pub icon: Memo<String>,
    pub size: Memo<Vec2>,
    pub modal: Memo<bool>,
    pub focused: Memo<bool>,
    pub attention: ReadSignal<bool>,
    pub closable: bool,
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
            self.set_dialog_focus.set(current);
        }
    }

    pub(super) fn release_dialog_focus(&self) {
        if self.dialog_focus.get_untracked().is_some() {
            self.set_dialog_focus.set(None);
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
        self.set_dialog_focus.set(Some(tab));
        self.settle_dialog_focus();
    }

    pub(super) fn call_attention(&self) {
        if let Some(dialog) = self.dialog_focus.get_untracked() {
            self.set_attention.update(|(count, target)| {
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
            .dialog_bodies.get()
            .borrow()
            .get(&tab)
            .and_then(NodeRef::try_get)
        else {
            return;
        };
        with_document(|document| {
            if document.focus_is_within(frame) {
                return;
            }
            if let Some(first) = document.focusables_within(frame).first().copied() {
                document.focus_focusable(first);
            }
        });
    }

    fn anchor_node(&self, parent: TabId) -> Option<NodeId> {
        if self.is_dialog(parent) {
            return self.dialog_frames.get().borrow().get(&parent)?.try_get();
        }
        let shown = self.state.with_untracked(|state| state.contains(parent));
        shown.then(|| self.panel(parent))
    }
}

#[component]
pub(super) fn DockDialogs(dock: State) -> NodeId {
    let keys = dock.dialogs;
    let settling = dock;
    let state = dock.state;
    let described = dock.dialog;
    create_effect(move || {
        let _ = state.with(DockState::focused_tab);
        for (dialog, _) in keys.get() {
            if let Some(parts) = described.call(dialog) {
                parts.modal.get();
            }
        }
        untrack(|| settling.settle_dialog_focus());
    });
    view! {
        <beui_view::reactive::List spacing=0.0>
            <beui_view::reactive::ForEach keys>
                {move |(tab, parent): (TabId, TabId)| {
                    view! {
                        <DockDialogView dock tab parent />
                    }
                }}
            </beui_view::reactive::ForEach>
        </beui_view::reactive::List>
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
        on_close,
        content,
    } = parts;
    let frame = NodeRef::new();
    dock.dialog_frames.get().borrow_mut().insert(tab, frame);
    let inside = NodeRef::new();
    dock.dialog_bodies.get().borrow_mut().insert(tab, inside);
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
            dock.set_dialog_focus.set(None);
        }
    });
    let shown = create_memo(move || dock.dialog_shown(tab));
    let anchor = NodeRef::new();
    create_effect(move || {
        if !shown.get() {
            return;
        }
        if let Some(node) = untrack(|| dock.anchor_node(parent)) {
            anchor.fill(node);
        }
    });
    let blocking = create_memo(move || shown.get() && modal.get());
    let policy = create_memo(move || match blocking.get() {
        true => LayerPolicy::BlocksParent,
        false => LayerPolicy::Pass,
    });
    let focus = dock.dialog_focus;
    let focused = create_memo(move || focus.get() == Some(tab));
    let (attention, set_attention) = create_signal(false);
    let calm = set_attention;
    let settle = create_timer(move || {
        calm.set(false);
        None
    });
    let requests = dock.attention;
    create_effect(move || {
        let (_, target) = requests.get();
        if target == Some(tab) {
            untrack(|| {
                set_attention.set(true);
                settle.start(ATTENTION);
            });
        }
    });
    let scope = with_document(|document| node_scope(document, dock.owner.with(Clone::clone)));
    let control = DockTabControl {
        tab,
        state: dock.state,
        dock,
    };
    let body = scope.context().run(|| {
        provide_context(control);
        content.call(())
    });
    on_cleanup(move || drop(scope));
    let width = create_memo(move || size.get().x);
    let height = create_memo(move || size.get().y);
    let body = view! {
        <Frame @node_ref=&inside @test_id={format!("dock.dialog.{}", tab.value())} width height>
            {body}
        </Frame>
    };
    let closing = on_close;
    let face = dock.dialog_chrome.call(DockDialogHandle {
        tab,
        title,
        icon,
        size,
        modal,
        focused,
        attention,
        closable: on_close.is_some(),
        close: ClickCallback::new(move || {
            if let Some(close) = &closing {
                close.call();
            }
        }),
        body,
    });
    let pressed = dock;
    let attending = dock;
    let dismissing = on_close;
    let modal_now = blocking;
    let overlay = NodeRef::new();
    let raised = overlay;
    let (opening, attended) = (shown, focused);
    let view = view! {
        <Overlay
            @node_ref=&overlay
            anchor={OverlayAnchor::Node(anchor)}
            placement=Placement::CenterOn
            policy
            traps_focus=false
            open={shown}
            on_dismiss={move |reason: Dismiss| match reason {
                Dismiss::OutsidePress if modal_now.get_untracked() => {
                    attending.focus_dialog(tab);
                    attending.focus_into_dialog();
                    attending.call_attention();
                }
                Dismiss::Escape | Dismiss::Back => {
                    if let Some(close) = &dismissing {
                        close.call();
                    }
                }
                Dismiss::OutsidePress | Dismiss::WithParent => {}
            }}
        >
            <Interactive
                @node_ref=&frame
                on_press={move |_: PointerPress| pressed.focus_dialog(tab)}
            >
                {face}
            </Interactive>
        </Overlay>
    };
    create_effect(move || {
        if !(blocking.get() || attended.get()) {
            return;
        }
        if let Some(overlay) = raised.try_get() {
            with_document(|document| document.raise_overlay(overlay));
        }
    });
    let focusing = dock;
    create_effect(move || {
        if attended.get() {
            untrack(|| focusing.focus_into_dialog());
        }
    });
    let opened = dock;
    let was_shown = std::cell::Cell::new(false);
    create_effect(move || {
        let now = opening.get();
        if now && !was_shown.replace(true) {
            untrack(|| opened.focus_dialog(tab));
        }
    });
    view
}
