use beui::NodeId;
use beui::reactive::{Align, Direction, Frame, List, Show, clone, component, create_memo, view};
use beui::styled::{Body, Button, ButtonVariant, use_theme};

use super::state::Shared;

const BAR_PADDING_HORIZONTAL: f32 = 12.0;
const BAR_PADDING_VERTICAL: f32 = 6.0;
const BAR_SPACING: f32 = 6.0;

#[component]
pub(crate) fn ImportError(state: Shared) -> NodeId {
    let error = state.import_error.clone();
    let shown = create_memo(clone!(error -> move || error.get().is_some()));
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                {move || view! {
                    <ImportErrorRow state={state.clone()} />
                }}
            </Show>
        </List>
    }
}

#[component]
fn ImportErrorRow(state: Shared) -> NodeId {
    let theme = use_theme();
    let error = state.import_error.clone();
    let message = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    view! {
        <Frame
            color={theme.surface.clone()}
            padding_horizontal=BAR_PADDING_HORIZONTAL
            padding_vertical=BAR_PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                <Body content={message} color={theme.danger.clone()} />
                <Button
                    label="Dismiss"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.image-error.dismiss"}
                    on_click={move || state.set_import_error.set(None)}
                />
            </List>
        </Frame>
    }
}
