mod find;

use beui_macros::{component, view};

use text_editor_core::{CopyMode, EditorCommand};

use crate::base::ItemSize;
use crate::geometry::Pos2;
use crate::input::KeyPress;
use crate::node::NodeId;
use crate::reactive::{
    Callback, CanvasItem, Children, ForEach, List, Prop, clone, copy_text, create_memo,
    create_signal,
};
use crate::styled::ContextMenu;
use crate::styled::theme::use_theme;
use crate::unstyled;
use crate::unstyled::{MenuItem, RemoteTextCursor, TextAreaColors, TextAreaState, TextWidget};

use find::FindBar;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MenuAction {
    Copy,
    Cut,
    SelectAll,
}

impl MenuAction {
    const ALL: [Self; 3] = [Self::Copy, Self::Cut, Self::SelectAll];

    fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::SelectAll => "Select All",
        }
    }
}

fn menu_action(state: &TextAreaState, action: MenuAction) {
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
        MenuAction::SelectAll => state.execute(EditorCommand::SelectAll),
    }
}

#[component]
pub fn TextArea(
    state: TextAreaState,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    on_widget_press: Callback<usize, bool>,
    on_key_override: Callback<KeyPress, bool>,
    children: Children<CanvasItem>,
) -> NodeId {
    let theme = use_theme();
    let colors = create_memo(move || TextAreaColors {
        selection: theme.accent_soft.get(),
        caret: theme.accent.get(),
        ..TextAreaColors::DEFAULT
    });
    let (menu_at, set_menu_at) = create_signal(None::<Pos2>);
    let menu_state = state.clone();
    let close_menu = set_menu_at.clone();
    view! {
        <List spacing=0.0>
            <FindBar state={state.clone()} />
            <ContextMenu
                @sizing=ItemSize::Percent(100.0)
                child_size=ItemSize::Percent(100.0)
                open_at={menu_at}
                on_close={move || close_menu.set(None)}
                items={view! {
                    <ForEach keys={MenuAction::ALL.to_vec()}>
                        {move |action: MenuAction| view! {
                            <MenuItem label={action.label().to_owned()} />
                        }}
                    </ForEach>
                }}
                on_select={clone!(menu_state -> move |path: Vec<usize>| {
                    let Some(action) = path
                        .first()
                        .and_then(|index| MenuAction::ALL.get(*index).copied())
                    else {
                        return;
                    };
                    menu_action(&menu_state, action);
                })}
            >
                <unstyled::TextArea
                    state={state}
                    widgets
                    colors
                    remote_cursors
                    drop_caret
                    on_widget_press={move |widget: usize| on_widget_press.call(widget)}
                    on_key_override={move |press: KeyPress| on_key_override.call(press)}
                    on_menu={move |at: Pos2| set_menu_at.set(Some(at))}
                >
                    {children}
                </unstyled::TextArea>
            </ContextMenu>
        </List>
    }
}
