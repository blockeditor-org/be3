use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use super::state::State;

const BAR_PADDING_HORIZONTAL: f32 = 12.0;
const BAR_PADDING_VERTICAL: f32 = 6.0;
const BAR_SPACING: f32 = 6.0;

#[component]
pub(crate) fn ImportError(state: State) -> NodeId {
    let error = state.import_error;
    let shown = create_memo(move || error.get().is_some());
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                {move || view! {
                    <ImportErrorRow state={state} />
                }}
            </Show>
        </List>
    }
}

#[component]
fn ImportErrorRow(state: State) -> NodeId {
    let error = state.import_error;
    let message = create_memo(move || error.get().unwrap_or_default());
    view! {
        <Frame
            fill=ColorRole::Surface
            padding={Sides::symmetric(BAR_PADDING_HORIZONTAL, BAR_PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                <Text string={message} color=TextColor::Danger />
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
