use accesskit::{Node, Role};
use beui_macros::{component, view};

use super::modal::{Modal, ModalHandle};
use beui_core::color::Color32;
use beui_core::input::Key;
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::NodeId;
use beui_view::components::portal::Portal;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
        KeyAt::Node(beui_core::tree::component_node()),
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
}

pub struct DialogSheetHandle {
    pub open: Memo<bool>,
    pub close: ClickCallback,
    pub body: Child,
}

#[component]
pub fn Dialog(
    open: Prop<bool>,
    kind: DialogKind,
    scrim: Prop<Color32>,
    sheet_below: f32,
    #[prop(default = false)] narrow: Prop<bool>,
    on_dismiss: ClickCallback,
    card: Render<Child>,
    sheet: Render<DialogSheetHandle>,
    body: Render<DialogLayout>,
) -> NodeId {
    let open = create_memo(move || open.get());
    let screens = use_screens();
    let narrow_screen = create_memo(move || {
        let widest = screens.with(|screens| {
            screens
                .iter()
                .map(|screen| screen.rect.width())
                .fold(0.0_f32, f32::max)
        });
        widest > 0.0 && widest < sheet_below
    });
    let touch = use_touch();
    let touched = create_signal(false);
    create_effect(move || {
        if open.get() {
            let touched_now = untrack(|| touch.get());
            untrack(|| touched.set(touched_now));
        }
    });
    let narrow = create_memo(move || narrow.get() || narrow_screen.get() || touched.get());
    let sheeted = create_memo(move || kind == DialogKind::Dialog && narrow.get());
    let layout = DialogLayout {
        narrow,
        sheet: sheeted,
    };
    provide_context(layout);
    let mut ever = false;
    let opened = create_memo(move || {
        ever |= open.get();
        ever
    });
    let built = create_signal(None::<NodeId>);
    let pending = std::cell::Cell::new(Some(body));
    let scope = owner_scope().expect("a dialog is built inside a scope");
    create_binding(move || {
        if opened.get()
            && let Some(body) = pending.take()
        {
            let node = untrack(|| scope.run(|| body.call(layout)));
            built.set(Some(node));
        }
    });
    let in_card = create_memo(move || (!sheeted.get()).then(|| built.get()).flatten());
    let in_sheet = create_memo(move || sheeted.get().then(|| built.get()).flatten());
    let ready = create_memo(move || open.get() && built.with(Option::is_some));
    let as_card = create_memo(move || ready.get() && !sheeted.get());
    let as_sheet = create_memo(move || ready.get() && sheeted.get());
    let sheet_body = view! {
        <Portal node={in_sheet} />
    };
    let sheet = sheet.call(DialogSheetHandle {
        open: as_sheet,
        close: on_dismiss,
        body: sheet_body,
    });
    view! {
        <List spacing=0.0>
            <Modal open={as_card} scrim on_dismiss={on_dismiss}>|_: ModalHandle| {
                let portal = view! {
                    <Portal node={in_card} />
                };
                card.call(portal)
            }</Modal>
            {sheet}
        </List>
    }
}
