use beui_macros::{component, view};

use beui_text_editor_core::{CopyMode, EditorCommand};

use crate::ContextMenu;
use crate::MenuItem;
use crate::TextAreaState;
use crate::menu_popup::MenuStyle;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Rect;

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
    menu: Option<MenuStyle>,
    #[prop(default = false)] masked: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] read_only: Prop<bool>,
    #[prop(default = 0.0)] child_grow: Prop<f32>,
    #[prop(children)] content: Render<Callback<Option<Rect>>>,
) -> NodeId {
    let Some(menu) = menu else {
        let content = content.call(Callback::new(|_: Option<Rect>| ()));
        return view! {
            <List spacing=0.0>
                <ListItem grow=child_grow>{content}</ListItem>
            </List>
        };
    };
    let requested = create_signal(None::<(Rect, u64)>);
    let cursors = state.cursors();
    let toolbar_at = create_memo(move || {
        requested
            .get()
            .filter(|(_, version)| *version == cursors.get())
            .map(|(rect, _)| rect)
    });
    let actions = create_memo(move || {
        cursors.get();
        let selected = state
            .selection_ranges()
            .iter()
            .any(|range| !range.is_empty());
        let copies = selected && !masked.get();
        let edits = !read_only.get();
        [
            (MenuAction::Copy, copies),
            (MenuAction::Cut, copies && edits),
            (MenuAction::Paste, edits),
            (MenuAction::SelectAll, true),
        ]
        .into_iter()
        .filter_map(|(action, shown)| shown.then_some(action))
        .collect::<Vec<_>>()
    });
    let off = create_memo(move || disabled.get());
    let open = Callback::new(move |at: Option<Rect>| {
        let at = at.filter(|_| !off.get_untracked());
        requested.set(at.map(|rect| (rect, cursors.get_untracked())));
    });
    view! {
        <ContextMenu
            menu
            child_grow
            disabled={off}
            selection=true
            toolbar_at
            opens_on_hold=false
            on_toolbar_close={move || requested.set(None)}
            items={view! {
                <ForEach keys={actions}>|action: MenuAction|
                    <MenuItem
                        label={action.label().to_owned()}
                        on_click={move || {
                            state.focus();
                            run(&state, action);
                        }}
                    />
                </ForEach>
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
            state.copy_to_clipboard(mode);
        }
        MenuAction::Paste => request_paste(),
        MenuAction::SelectAll => state.execute(EditorCommand::SelectAll),
    }
}
