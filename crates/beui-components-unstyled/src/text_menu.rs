use beui_macros::{component, view};

use text_editor_core::{CopyMode, EditorCommand};

use crate::ContextMenu;
use crate::MenuItem;
use crate::MenuRowHandle;
use crate::TextAreaState;
use beui_core::base::ItemSize;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Child, ClickCallback, ForEach, List, Prop, RenderFn, clone, copy_text,
    create_memo, request_paste,
};

#[derive(Clone, Default)]
pub struct TextMenu(Option<(RenderFn<MenuRowHandle>, RenderFn<Child>)>);

impl TextMenu {
    pub fn new(
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self(Some((RenderFn::new(row), RenderFn::new(panel))))
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

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
    menu: TextMenu,
    open_at: Prop<Option<Pos2>>,
    #[prop(default = false)] masked: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let cursors = state.cursors();
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
    let no_menu = menu.0.is_none();
    let (row, panel) = menu.0.unwrap_or_else(|| {
        (
            RenderFn::new(|_| {
                view! {
                    <List spacing=0.0 />
                }
            }),
            RenderFn::new(|content| content),
        )
    });
    let off = create_memo(move || no_menu || disabled.get());
    view! {
        <ContextMenu
            row
            panel
            child_size
            disabled={off}
            open_at
            open_at_focuses=false
            on_close={move || on_close.call()}
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
            {children}
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
