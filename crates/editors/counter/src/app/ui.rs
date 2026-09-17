use block_client::blocks::counter::{Counter as CounterBlock, CounterOperation};
use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    CenteredRow, Column, Frame, ItemSize, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Display, use_theme};

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

#[component]
pub fn Counter(editor: Editor) -> NodeId {
    let counter = editor.block::<CounterBlock>();
    let count = counter.project(CounterBlock::count);
    let shown = create_memo(clone!(count -> move || count.get().to_string()));
    let decrement = clone!(counter -> move || counter.operate(CounterOperation::Decrement));
    let increment = clone!(counter -> move || counter.operate(CounterOperation::Increment));
    let reset = clone!(counter -> move || counter.operate(CounterOperation::Reset));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <Column spacing=16.0>
                <Display content=shown @test_id="counter.value" />
                <CenteredRow spacing=10.0>
                    <Button
                        @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                        label="-"
                        variant=ButtonVariant::Primary
                        @test_id="counter.decrement"
                        on_click=decrement
                    />
                    <Button
                        @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                        label="+"
                        variant=ButtonVariant::Primary
                        @test_id="counter.increment"
                        on_click=increment
                    />
                    <Button
                        label="Reset"
                        variant=ButtonVariant::Secondary
                        @test_id="counter.reset"
                        on_click=reset
                    />
                </CenteredRow>
            </Column>
        </Frame>
    }
}
