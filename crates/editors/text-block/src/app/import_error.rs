use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use super::state::State;

const BAR_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Sm);
const BAR_SPACING: Space = Space::Sm;

#[component]
pub(crate) fn ImportError(state: State) -> NodeId {
    let error = state.import_error;
    let shown = create_memo(move || error.get().is_some());
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <ImportErrorRow state={state} />
            </Show>
        </List>
    }
}

#[component]
fn ImportErrorRow(state: State) -> NodeId {
    let error = state.import_error;
    let message = create_memo(move || error.get().unwrap_or_default());
    view! {
        <Frame fill=ColorRole::Surface padding=BAR_PADDING>
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                <Text color=TextColor::Danger>{message}</Text>
                <Button
                    label="Dismiss"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.image-error.dismiss"}
                    on_click={move || state.import_error.set(None)}
                />
            </List>
        </Frame>
    }
}
