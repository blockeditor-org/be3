use block_editor_plugin::Editor;
use block_editor_plugin::be_block::{Counter as CounterModel, CounterContent};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Display, use_theme};

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

#[component]
pub fn Counter(editor: Editor) -> NodeId {
    let counter = editor.block_content::<CounterContent>();
    let count = counter.project(|counter| counter.root().value());
    let shown = create_memo(clone!(count -> move || count.get().to_string()));
    let decrement = clone!(counter -> move || counter.operate(CounterModel::add(-1)));
    let increment = clone!(counter -> move || counter.operate(CounterModel::add(1)));
    let reset = clone!(counter -> move || {
        if let Some(edit) = counter.read(|content| content.root().reset()) {
            counter.operate(edit);
        }
    });
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=16.0>
                <Display content={shown} @test_id={"counter.value"} />
                <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                    <Button
                        @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                        label="-"
                        variant=ButtonVariant::Primary
                        @test_id={"counter.decrement"}
                        on_click={decrement}
                    />
                    <Button
                        @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                        label="+"
                        variant=ButtonVariant::Primary
                        @test_id={"counter.increment"}
                        on_click={increment}
                    />
                    <Button
                        label="Reset"
                        variant=ButtonVariant::Secondary
                        @test_id={"counter.reset"}
                        on_click={reset}
                    />
                </List>
            </List>
        </Frame>
    }
}
