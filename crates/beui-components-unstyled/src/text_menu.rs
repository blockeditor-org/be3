use beui_macros::{component, view};

use text_editor_core::{CopyMode, EditorCommand};

use crate::ContextMenu;
use crate::MenuItem;
use crate::TextAreaState;
use crate::context_menu::MenuStyle;
use beui_core::base::ItemSize;
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, ForEach, Prop, Render, clone, copy_text, create_memo, create_signal, request_paste,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum MenuAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
}

impl MenuAction {
    fn label(self) -> &'static str {
        match self {
            MenuAction::Copy => "Copy",
            MenuAction::Cut => "Cut",
            MenuAction::Paste => "Paste",
            MenuAction::SelectAll => "Select All",
        }
    }
}

#[component]
pub fn TextContextMenu(
    state: TextAreaState,
    menu: MenuStyle,
    #[prop(default = false)] masked: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    #[prop(children)] content: Render<Callback<Option<Rect>>>,
) -> NodeId {
    let (requested, set_requested) = create_signal(None::<(Rect, u64)>);
    let cursors = state.cursors();
    let shown = cursors.clone();
    let toolbar_at = create_memo(clone!(requested cursors -> move || {
        requested
            .get()
            .filter(|(_, version)| *version == cursors.get())
            .map(|(rect, _)| rect)
    }));
    let actions = create_memo(clone!(state -> move || {
        cursors.get();
        let selected = state.selection_ranges().iter().any(|range| !range.is_empty());
        match selected && !masked.get() {
            true => vec![
                MenuAction::Copy,
                MenuAction::Cut,
                MenuAction::Paste,
                MenuAction::SelectAll,
            ],
            false => vec![MenuAction::Paste, MenuAction::SelectAll],
        }
    }));
    let chosen = actions.clone();
    let no_menu = !menu.is_some();
    let off = create_memo(move || no_menu || disabled.get());
    let open = Callback::new(clone!(off set_requested -> move |at: Option<Rect>| {
        let at = at.filter(|_| !off.get_untracked());
        set_requested.set(at.map(|rect| (rect, shown.get_untracked())));
    }));
    view! {
        <ContextMenu
            menu
            child_size
            disabled={off}
            toolbar_at
            opens_on_hold=false
            on_toolbar_close={move || set_requested.set(None)}
            items={view! {
                <ForEach keys={actions}>
                    {move |action: MenuAction| view! {
                        <MenuItem label={action.label().to_owned()} />
                    }}
                </ForEach>
            }}
            on_select={move |path: Vec<usize>| {
                let Some(action) = path
                    .first()
                    .and_then(|index| chosen.get_untracked().get(*index).copied())
                else {
                    return;
                };
                state.focus();
                run(&state, action);
            }}
        >
            {content.call(open)}
        </ContextMenu>
    }
}

fn run(state: &TextAreaState, action: MenuAction) {
    match action {
        MenuAction::Copy | MenuAction::Cut => {
            let mode = match action {
                MenuAction::Cut => CopyMode::Cut,
                _ => CopyMode::Copy,
            };
            let text = state.copy(mode);
            if !text.is_empty() {
                copy_text(text);
            }
        }
        MenuAction::Paste => request_paste(),
        MenuAction::SelectAll => state.execute(EditorCommand::SelectAll),
    }
}
