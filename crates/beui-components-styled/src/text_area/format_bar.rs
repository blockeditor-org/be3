use beui_macros::{component, view};

use crate::ToggleButton;
use beui_components_unstyled::TextAreaState;
use beui_core::icons::{
    ICON_CODE, ICON_FORMAT_BOLD, ICON_FORMAT_H1, ICON_FORMAT_H2, ICON_FORMAT_H3,
    ICON_FORMAT_ITALIC, ICON_FORMAT_STRIKETHROUGH, ICON_FORMAT_UNDERLINED,
};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;
use text_editor_core::{EditorCommand, RichStyle};

const BAR_SPACING: f32 = 4.0;

#[component]
pub fn FormatBar(
    state: TextAreaState,
    #[prop(default = "format")] test_prefix: &'static str,
) -> NodeId {
    let toggle = |style: RichStyle, glyph: &'static str, label: &'static str, name: &str| {
        let id = format!("{test_prefix}.{name}");
        view! {
            <FormatToggle state style glyph label id />
        }
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING wrap=true>
            {toggle(RichStyle::Bold, ICON_FORMAT_BOLD, "Bold", "bold")}
            {toggle(RichStyle::Italic, ICON_FORMAT_ITALIC, "Italic", "italic")}
            {toggle(RichStyle::Underline, ICON_FORMAT_UNDERLINED, "Underline", "underline")}
            {toggle(
                RichStyle::Strikethrough,
                ICON_FORMAT_STRIKETHROUGH,
                "Strikethrough",
                "strikethrough",
            )}
            {toggle(RichStyle::Code, ICON_CODE, "Code", "code")}
            {toggle(RichStyle::Heading(1), ICON_FORMAT_H1, "Heading 1", "heading1")}
            {toggle(RichStyle::Heading(2), ICON_FORMAT_H2, "Heading 2", "heading2")}
            {toggle(RichStyle::Heading(3), ICON_FORMAT_H3, "Heading 3", "heading3")}
        </List>
    }
}

#[component]
fn FormatToggle(
    state: TextAreaState,
    style: RichStyle,
    glyph: &'static str,
    label: &'static str,
    id: String,
) -> NodeId {
    let format = state.format();
    let pressed = create_memo(move || format.get().is_some_and(|format| style.applies_to(format)));
    let disabled = create_memo(move || format.get().is_none());
    view! {
        <ToggleButton
            @test_id=id
            glyph
            icon_only=true
            label
            pressed={pressed}
            disabled={disabled}
            on_change={move |_: bool| {
                state.execute(EditorCommand::ToggleStyle(style));
                state.focus();
            }}
        />
    }
}
