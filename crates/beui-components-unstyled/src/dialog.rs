use accesskit::{Node, Role};
use beui_macros::{component, view};

use super::modal::{Modal, ModalHandle};
use beui_view::components::overlay::Overlay;
use beui_view::components::portal::Portal;
use beui_view::primitives::*;
use beui_view::reactive::KeyAt;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Key;
use beui_view::types::KeyPhase;
use beui_view::types::{Dismiss, OverlayAnchor, Placement, Pos2, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DialogKind {
    Alert,
    Dialog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionRole {
    Primary,
    Destructive,
    Secondary,
    Cancel,
}

#[derive(Clone, Copy)]
pub struct DialogAction {
    pub label: Memo<String>,
    pub role: ActionRole,
    pub disabled: Memo<bool>,
    pub busy: Memo<bool>,
    pub shown: Memo<bool>,
    pub name: &'static str,
    pub on_click: ClickCallback,
}

impl DialogAction {
    pub fn new(
        role: ActionRole,
        label: impl IntoProp<String>,
        on_click: impl IntoClickCallback,
    ) -> Self {
        let label = label.into_prop();
        Self {
            label: create_memo(move || label.get()),
            role,
            disabled: create_memo(|| false),
            busy: create_memo(|| false),
            shown: create_memo(|| true),
            name: match role {
                ActionRole::Primary | ActionRole::Destructive => "confirm",
                ActionRole::Cancel => "cancel",
                ActionRole::Secondary => "secondary",
            },
            on_click: on_click.into_click_callback(),
        }
    }

    pub fn primary(label: impl IntoProp<String>, on_click: impl IntoClickCallback) -> Self {
        Self::new(ActionRole::Primary, label, on_click)
    }

    pub fn destructive(label: impl IntoProp<String>, on_click: impl IntoClickCallback) -> Self {
        Self::new(ActionRole::Destructive, label, on_click)
    }

    pub fn secondary(label: impl IntoProp<String>, on_click: impl IntoClickCallback) -> Self {
        Self::new(ActionRole::Secondary, label, on_click)
    }

    pub fn cancel(label: impl IntoProp<String>) -> Self {
        Self::new(ActionRole::Cancel, label, ClickCallback::empty())
    }

    pub fn disabled(mut self, disabled: impl IntoProp<bool>) -> Self {
        let disabled = disabled.into_prop();
        self.disabled = create_memo(move || disabled.get());
        self
    }

    pub fn busy(mut self, busy: impl IntoProp<bool>) -> Self {
        let busy = busy.into_prop();
        self.busy = create_memo(move || busy.get());
        self
    }

    pub fn shown(mut self, shown: impl IntoProp<bool>) -> Self {
        let shown = shown.into_prop();
        self.shown = create_memo(move || shown.get());
        self
    }

    pub fn named(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    pub fn enabled(&self) -> bool {
        self.shown.get_untracked() && !self.disabled.get_untracked() && !self.busy.get_untracked()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FooterOrder {
    pub start: Vec<usize>,
    pub end: Vec<usize>,
}

impl FooterOrder {
    pub fn is_empty(&self) -> bool {
        self.start.is_empty() && self.end.is_empty()
    }
}

fn shown(actions: &[DialogAction]) -> Vec<(usize, ActionRole)> {
    actions
        .iter()
        .enumerate()
        .filter(|(_, action)| action.shown.get())
        .map(|(index, action)| (index, action.role))
        .collect()
}

fn main_action(shown: &[(usize, ActionRole)]) -> Option<usize> {
    let of = |wanted: ActionRole| {
        shown
            .iter()
            .find(|(_, role)| *role == wanted)
            .map(|(index, _)| *index)
    };
    of(ActionRole::Primary).or_else(|| of(ActionRole::Destructive))
}

pub fn footer_order(actions: &[DialogAction], narrow: bool) -> FooterOrder {
    let shown = shown(actions);
    let main = main_action(&shown);
    let others: Vec<usize> = shown
        .iter()
        .filter(|(index, role)| *role != ActionRole::Cancel && Some(*index) != main)
        .map(|(index, _)| *index)
        .collect();
    let cancel: Vec<usize> = shown
        .iter()
        .filter(|(_, role)| *role == ActionRole::Cancel)
        .map(|(index, _)| *index)
        .collect();
    match narrow {
        true => FooterOrder {
            start: Vec::new(),
            end: main.into_iter().chain(others).chain(cancel).collect(),
        },
        false => FooterOrder {
            start: others,
            end: cancel.into_iter().chain(main).collect(),
        },
    }
}

#[derive(Clone, Copy)]
pub struct DialogBodyHandle {
    pub kind: DialogKind,
    pub title: Memo<String>,
    pub narrow: Memo<bool>,
    pub actions: Stored<Vec<DialogAction>>,
    pub footer: Memo<FooterOrder>,
    pub focus: Memo<Option<usize>>,
    pub dismiss: ClickCallback,
    pub run: Callback<usize>,
}

impl DialogBodyHandle {
    pub fn action(&self, index: usize) -> DialogAction {
        self.actions.with(|actions| actions[index])
    }
}

#[component]
pub fn DialogBody(
    kind: DialogKind,
    title: Prop<String>,
    actions: Vec<DialogAction>,
    #[prop(default = false)] narrow: Prop<bool>,
    #[prop(default = true)] focuses: Prop<bool>,
    on_dismiss: ClickCallback,
    face: Render<DialogBodyHandle>,
) -> NodeId {
    let actions = store(actions);
    let title = create_memo(move || title.get());
    let narrow = create_memo(move || narrow.get());
    let footer = create_memo(move || actions.with(|actions| footer_order(actions, narrow.get())));
    let focus = create_memo(move || {
        if kind != DialogKind::Alert || !focuses.get() {
            return None;
        }
        let shown = actions.with(|actions| shown(actions));
        let main = main_action(&shown)?;
        match actions.with(|actions| actions[main].role) {
            ActionRole::Primary => Some(main),
            _ => shown
                .iter()
                .find(|(_, role)| *role == ActionRole::Cancel)
                .map(|(index, _)| *index),
        }
    });
    if let Some(dismissal) = use_context::<DialogDismissal>() {
        dismissal.claim(on_dismiss);
    }
    let run = Callback::new(move |index: usize| {
        let Some(action) = actions.with(|actions| actions.get(index).copied()) else {
            return;
        };
        match action.role {
            ActionRole::Cancel => on_dismiss.call(),
            _ => action.on_click.call(),
        }
    });
    component_accessibility(create_memo(move || {
        let mut node = Node::new(match kind {
            DialogKind::Alert => Role::AlertDialog,
            DialogKind::Dialog => Role::Dialog,
        });
        let title = title.get();
        if !title.is_empty() {
            node.set_label(title);
        }
        node
    }));
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        move |event| {
            event.press().is_some_and(|press| {
                press.key == Key::Enter
                    && press.pressed
                    && !press.repeat
                    && !press.modifiers.ctrl
                    && !press.modifiers.alt
                    && !press.modifiers.shift
                    && actions.with(|actions| enter(actions, run))
            })
        },
    );
    face.call(DialogBodyHandle {
        kind,
        title,
        narrow,
        actions,
        footer,
        focus,
        dismiss: on_dismiss,
        run,
    })
}

fn enter(actions: &[DialogAction], run: Callback<usize>) -> bool {
    let shown = untrack(|| shown(actions));
    let Some(main) = main_action(&shown) else {
        return false;
    };
    let action = actions[main];
    if action.role != ActionRole::Primary || !action.enabled() {
        return false;
    }
    run.call(main);
    true
}

#[derive(Clone, Copy)]
pub struct DialogLayout {
    pub narrow: Memo<bool>,
    pub sheet: Memo<bool>,
    pub bottom_inset: Memo<f32>,
}

#[derive(Clone, Copy)]
pub struct DialogDismissal {
    claimed: Signal<Option<u64>>,
    body: StoredCell<Option<ClickCallback>>,
    next: StoredCell<u64>,
}

impl DialogDismissal {
    pub fn new() -> Self {
        Self {
            claimed: create_signal(None),
            body: StoredCell::new(None),
            next: StoredCell::new(0),
        }
    }

    pub fn handled(&self) -> bool {
        self.claimed.with(Option::is_some)
    }

    pub fn dismiss(&self) -> bool {
        match self.body.get() {
            Some(dismiss) => {
                dismiss.call();
                true
            }
            None => false,
        }
    }

    fn claim(self, dismiss: ClickCallback) {
        let key = self.next.get();
        self.next.set(key + 1);
        self.body.set(Some(dismiss));
        self.claimed.set(Some(key));
        on_cleanup(move || {
            if self.claimed.is_alive()
                && self.body.is_alive()
                && self.claimed.get_untracked() == Some(key)
            {
                self.body.set(None);
                self.claimed.set(None);
            }
        });
    }
}

impl Default for DialogDismissal {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DialogSheetHandle {
    pub open: Memo<bool>,
    pub close: ClickCallback,
    pub tall: bool,
    pub body: Child,
}

pub struct DialogCardHandle {
    pub pointer: NodeRef,
    pub body: Child,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DialogSize {
    #[default]
    Small,
    Large,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogAnchor {
    #[default]
    None,
    Control(NodeRef),
    Frame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogPresentation {
    Centred,
    Anchored(NodeId),
    HangsFromFrame,
    OverFrame,
    Sheet { tall: bool },
}

pub fn dialog_presentation(
    kind: DialogKind,
    size: DialogSize,
    anchor: DialogAnchor,
    opener: Option<NodeId>,
    narrow: bool,
) -> DialogPresentation {
    match (kind, narrow, anchor, size) {
        (DialogKind::Alert, _, _, _) => DialogPresentation::Centred,
        (DialogKind::Dialog, true, _, size) => DialogPresentation::Sheet {
            tall: size == DialogSize::Large,
        },
        (DialogKind::Dialog, false, DialogAnchor::None, _) => DialogPresentation::Centred,
        (DialogKind::Dialog, false, _, DialogSize::Large) => DialogPresentation::OverFrame,
        (DialogKind::Dialog, false, DialogAnchor::Control(_), DialogSize::Small) => match opener {
            Some(node) => DialogPresentation::Anchored(node),
            None => DialogPresentation::HangsFromFrame,
        },
        (DialogKind::Dialog, false, DialogAnchor::Frame, DialogSize::Small) => {
            DialogPresentation::HangsFromFrame
        }
    }
}

const DIALOG_GAP: u16 = 4;
const DIALOG_MARGIN: u16 = 8;

#[component]
pub fn Dialog(
    open: Prop<bool>,
    kind: DialogKind,
    scrim: Prop<Color>,
    sheet_below: f32,
    #[prop(default = false)] narrow: Prop<bool>,
    #[prop(default = DialogSize::Small)] size: DialogSize,
    #[prop(default = DialogAnchor::None)] anchor: Prop<DialogAnchor>,
    #[prop(default = 0.0)] beak: f32,
    on_dismiss: ClickCallback,
    card: Render<DialogCardHandle>,
    sheet: Render<DialogSheetHandle>,
    body: Render<DialogLayout>,
) -> NodeId {
    let open = create_memo(move || open.get());
    let viewport = use_viewport_size();
    let narrow_screen = create_memo(move || {
        let width = viewport.get().x;
        width > 0.0 && width < sheet_below
    });
    let touched = create_signal(false);
    let opener = create_signal(None::<NodeId>);
    let asked = anchor.clone();
    create_effect(move || {
        if open.get() {
            let touched_now = untrack(touched_by_finger);
            let found = untrack(|| match asked.get() {
                DialogAnchor::Control(control) => control.try_get().map(opener_of),
                DialogAnchor::None | DialogAnchor::Frame => None,
            });
            untrack(|| {
                touched.set(touched_now);
                opener.set(found);
            });
        }
    });
    let narrow = create_memo(move || narrow.get() || narrow_screen.get() || touched.get());
    let presentation = create_memo(move || {
        dialog_presentation(kind, size, anchor.get(), opener.get(), narrow.get())
    });
    let sheeted =
        create_memo(move || matches!(presentation.get(), DialogPresentation::Sheet { .. }));
    let centred = create_memo(move || presentation.get() == DialogPresentation::Centred);
    let layout = DialogLayout {
        narrow,
        sheet: sheeted,
        bottom_inset: create_memo(|| 0.0),
    };
    provide_context(layout);
    let mut ever = false;
    let opened = create_memo(move || {
        ever |= open.get();
        ever
    });
    let built = create_signal(None::<NodeId>);
    let card_built = create_signal(None::<NodeId>);
    let in_sheet = create_memo(move || sheeted.get().then(|| built.get()).flatten());
    let in_card = create_memo(move || (!sheeted.get()).then(|| built.get()).flatten());
    let pointer = NodeRef::new();
    let pending = std::cell::Cell::new(Some((body, card)));
    let scope = owner_scope().expect("a dialog is built inside a scope");
    create_binding(move || {
        if opened.get()
            && let Some((body, card)) = pending.take()
        {
            let (node, card) = untrack(|| {
                scope.run(|| {
                    let node = body.call(layout);
                    let card = card.call(DialogCardHandle {
                        pointer,
                        body: view! {
                            <Portal node={in_card} />
                        },
                    });
                    (node, card)
                })
            });
            built.set(Some(node));
            card_built.set(Some(card));
        }
    });
    let centred_card = create_memo(move || centred.get().then(|| card_built.get()).flatten());
    let layer_card = create_memo(move || {
        (!centred.get() && !sheeted.get())
            .then(|| card_built.get())
            .flatten()
    });
    let ready = create_memo(move || open.get() && built.with(Option::is_some));
    let as_card = create_memo(move || ready.get() && centred.get());
    let as_sheet = create_memo(move || ready.get() && sheeted.get());
    let as_layer = create_memo(move || ready.get() && !centred.get() && !sheeted.get());
    let sheet_body = view! {
        <Portal node={in_sheet} />
    };
    let sheet = sheet.call(DialogSheetHandle {
        open: as_sheet,
        close: on_dismiss,
        tall: size == DialogSize::Large,
        body: sheet_body,
    });
    let held = NodeRef::new();
    let frame = create_memo(move || Rect::from_min_size(Pos2::ZERO, viewport.get()));
    let placed = create_memo(move || match presentation.get() {
        DialogPresentation::Anchored(node) => {
            if held.try_get() != Some(node) {
                held.fill(node);
            }
            (
                OverlayAnchor::Node(held),
                Placement::Popover {
                    gap: DIALOG_GAP,
                    margin: DIALOG_MARGIN,
                },
                false,
            )
        }
        DialogPresentation::HangsFromFrame => (
            OverlayAnchor::Rect(frame.get()),
            Placement::HangFrom(DIALOG_MARGIN),
            true,
        ),
        DialogPresentation::Centred
        | DialogPresentation::OverFrame
        | DialogPresentation::Sheet { .. } => {
            (OverlayAnchor::Rect(frame.get()), Placement::CenterOn, true)
        }
    });
    let at = create_memo(move || placed.get().0);
    let placement = create_memo(move || placed.get().1);
    let dims = scrim.clone();
    let layer_scrim = create_memo(move || match placed.get().2 {
        true => dims.get(),
        false => Color::TRANSPARENT,
    });
    let reach = create_memo(move || match placed.get().1 {
        Placement::Popover { .. } => beak,
        _ => 0.0,
    });
    view! {
        <List spacing=0.0>
            <Modal open={as_card} scrim on_dismiss={on_dismiss}>|_: ModalHandle|
                <Portal node={centred_card} />
            </Modal>
            <Overlay
                anchor={at}
                placement={placement}
                scrim={layer_scrim}
                pointer={pointer}
                beak={reach}
                open={as_layer}
                keep_alive=true
                on_dismiss={move |_: Dismiss| on_dismiss.call()}
            >
                <Portal node={layer_card} />
            </Overlay>
            {sheet}
        </List>
    }
}
