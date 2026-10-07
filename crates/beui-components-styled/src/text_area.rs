pub mod emoji;
pub mod find;

use beui_macros::{component, view};

use crate::checkbox::Checkbox;
use crate::context_menu::menu_style;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    Completer, RemoteTextCursor, TextAreaColors, TextAreaState, TextCheckbox, TextWidget,
};
use beui_core::base::ItemSize;
use beui_core::document::Document;
use beui_core::geometry::Pos2;
use beui_core::input::KeyPress;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Frame, List, NodeRef, Prop, RenderFn, create_memo, set_component_state,
};
use emoji::emoji_menu;

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
    let masked = password.clone();
    let menu_state = state.clone();
    let surface = NodeRef::new();
    set_component_state(surface.clone());
    let block = block.unwrap_or_else(blank_widget);
    let selected_widget = selected_widget.unwrap_or_else(blank_widget);
    view! {
        <List spacing=0.0>
            <FindBar state={state.clone()} />
            <unstyled::TextContextMenu
                @sizing=ItemSize::Percent(100.0)
                state={menu_state}
                menu={menu_style()}
                masked
                child_size=ItemSize::Percent(100.0)
            >
                {move |open_menu: Callback<Pos2>| view! {
                    <unstyled::TextArea
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
                        on_menu={move |at: Pos2| open_menu.call(at)}
                        block={block}
                        selected_widget={selected_widget}
                        checkbox={inline_checkbox()}
                        completer={match emoji {
                            true => unstyled::emoji_completer(),
                            false => Completer::none(),
                        }}
                        completion_menu={emoji_menu()}
                    />
                }}
            </unstyled::TextContextMenu>
        </List>
    }
}

fn inline_checkbox() -> RenderFn<TextCheckbox> {
    RenderFn::new(|checkbox: TextCheckbox| {
        let TextCheckbox {
            checked,
            disabled,
            on_change,
        } = checkbox;
        view! {
            <Checkbox
                checked
                disabled
                capture_presses=true
                tab_stop=false
                press_focus=false
                on_change={move |checked: bool| on_change.call(checked)}
            />
        }
    })
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
