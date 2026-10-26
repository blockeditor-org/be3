use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::ToggleButton;
use beui_components_unstyled::TextAreaState;
use beui_view::reactive::NodeId;

use crate::list::List;
use crate::tokens::Space;
use beui_view::reactive::*;
use text_editor_core::Tool;

const BAR_SPACING: Space = Space::Xs;

#[component]
pub fn FormatBar(
    state: TextAreaState,
    glyphs: Vec<(&'static str, Glyph)>,
    #[prop(default = "format")] test_prefix: &'static str,
) -> NodeId {
    let schema = state.schema();
    let tools: Vec<(Tool, Glyph)> = glyphs
        .iter()
        .filter_map(|(name, glyph)| {
            let tool = schema.tools().iter().find(|tool| tool.name == *name)?;
            Some((*tool, *glyph))
        })
        .collect();
    let keys: Vec<usize> = (0..tools.len()).collect();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING wrap=true>
            <ForEach keys={keys}>|index: usize| {
                let (tool, glyph) = tools[index];
                let id = format!("{test_prefix}.{}", tool.name);
                view! {
                    <FormatToggle state tool glyph id />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn FormatToggle(state: TextAreaState, tool: Tool, glyph: Glyph, id: String) -> NodeId {
    let selected = state.selected();
    let pressed = create_memo(move || {
        selected.with(|selected| {
            selected
                .as_ref()
                .is_some_and(|selected| (tool.active)(selected))
        })
    });
    let disabled = create_memo(move || selected.with(Option::is_none));
    view! {
        <ToggleButton
            @test_id=id
            glyph
            icon_only=true
            label={tool.label}
            pressed={Binding::new(pressed, move |_: bool| {
                if let Some(selected) = selected.get_untracked() {
                    state.execute((tool.command)(&selected));
                }
                state.focus();
            })}
            disabled={disabled}
        />
    }
}
