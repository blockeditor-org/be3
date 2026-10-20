pub mod emoji;
pub mod find;
pub mod format_bar;

use crate::frame::Frame;
use beui_macros::{component, view};

use crate::checkbox::Checkbox;
use crate::context_menu::menu_style;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    BlockView, Completer, RemoteTextCursor, TextAreaColors, TextAreaState, TextCheckbox,
    TextWidget, VoidSize, VoidView,
};
use beui_core::base::ItemSize;
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::file_picker::PickedFile;
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;
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
    #[prop(default = false)] show_runs: Prop<bool>,
    #[prop(default = Vec::new())] client_colors: Prop<Vec<(u64, Color32)>>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    render_block: Option<RenderFn<BlockView>>,
    render_void: Option<RenderFn<VoidView>>,
    #[prop(default = None)] void_size: Option<VoidSize>,
    #[prop(default = true)] emoji: bool,
    #[prop(default = false)] pastes_images: bool,
    on_widget_press: Callback<usize, bool>,
    on_focus_change: Callback<bool>,
    on_image_paste: Callback<PickedFile>,
) -> NodeId {
    let theme = use_theme();
    let colors = create_memo(move || TextAreaColors {
        selection: theme.accent_soft.get(),
        caret: theme.accent.get(),
        ..TextAreaColors::DEFAULT
    });
    let surface = NodeRef::new();
    set_component_state(surface);
    let block = block.unwrap_or_else(blank_widget);
    let selected_widget = selected_widget.unwrap_or_else(blank_widget);
    let render_block = render_block.unwrap_or_else(|| RenderFn::new(|block: BlockView| block.text));
    let render_void = render_void.unwrap_or_else(|| {
        RenderFn::new(|_: VoidView| {
            view! {
                <Frame />
            }
        })
    });
    view! {
        <List spacing=0.0>
            <FindBar state={state} />
            <unstyled::TextContextMenu
                @sizing=ItemSize::Percent(100.0)
                state={state}
                menu={menu_style()}
                masked={password}
                child_size=ItemSize::Percent(100.0)
            >|open_menu: Callback<Option<Rect>>|
                <unstyled::TextArea
                    @node_ref=&surface
                    state={state}
                    widgets
                    colors
                    remote_cursors
                    drop_caret
                    placeholder
                    password
                    show_runs
                    client_colors
                    on_widget_press={on_widget_press}
                    on_focus_change={on_focus_change}
                    pastes_images
                    on_image_paste={on_image_paste}
                    on_toolbar={move |at: Option<Rect>| open_menu.call(at)}
                    block={block}
                    selected_widget={selected_widget}
                    render_block={render_block}
                    render_void={render_void}
                    void_size
                    checkbox={inline_checkbox()}
                    completer={match emoji {
                        true => unstyled::emoji_completer(),
                        false => Completer::none(),
                    }}
                    completion_menu={emoji_menu()}
                />
            </unstyled::TextContextMenu>
        </List>
    }
}

fn inline_checkbox() -> RenderFn<TextCheckbox> {
    RenderFn::new(|checkbox: TextCheckbox| {
        let TextCheckbox { checked, disabled } = checkbox;
        view! {
            <Checkbox checked disabled capture_presses=true tab_stop=false press_focus=false />
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
