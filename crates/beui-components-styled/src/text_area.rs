pub mod emoji;
pub mod find;

use beui_macros::{component, view};

use crate::context_menu::text_menu;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    Completer, CompletionMenu, RemoteTextCursor, TextAreaColors, TextAreaState,
    TextWidget,
};
use beui_core::base::ItemSize;
use beui_core::document::Document;
use beui_core::input::KeyPress;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Frame, List, NodeRef, Prop, RenderFn, create_memo, set_component_state,
};
use emoji::EmojiMenu;

use find::FindBar;

#[component]
pub fn TextArea(
    state: TextAreaState,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] password: Prop<bool>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    #[prop(default = true)] emoji: bool,
    on_widget_press: Callback<usize, bool>,
    on_key_override: Callback<KeyPress, bool>,
    on_focus_change: Callback<bool>,
) -> NodeId {
    let theme = use_theme();
    let colors = create_memo(move || TextAreaColors {
        selection: theme.accent_soft.get(),
        caret: theme.accent.get(),
        ..TextAreaColors::DEFAULT
    });
    let surface = NodeRef::new();
    set_component_state(surface.clone());
    let block = block.unwrap_or_else(blank_widget);
    let selected_widget = selected_widget.unwrap_or_else(blank_widget);
    view! {
        <List spacing=0.0>
            <FindBar state={state.clone()} />
            <unstyled::TextArea
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref=&surface
                    state={state}
                    widgets
                    colors
                    remote_cursors
                    drop_caret
                    placeholder
                    password
                    on_widget_press={move |widget: usize| on_widget_press.call(widget)}
                    on_key_override={move |press: KeyPress| on_key_override.call(press)}
                    on_focus_change={move |focused: bool| on_focus_change.call(focused)}
                    menu={text_menu()}
                    block={block}
                    selected_widget={selected_widget}
                    completer={match emoji {
                        true => emoji::emoji_completer(),
                        false => Completer::none(),
                    }}
                    completion_menu={move |menu: CompletionMenu| view! {
                        <EmojiMenu menu />
                    }}
                />
        </List>
    }
}

fn blank_widget() -> RenderFn<usize> {
    RenderFn::new(|_| {
        view! {
            <Frame />
        }
    })
}

pub fn text_area_surface(document: &Document, area: NodeId) -> NodeId {
    document.component_state::<NodeRef>(area).get()
}
