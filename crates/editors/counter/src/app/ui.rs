use block_editor_beui::Editor;
use block_editor_beui::be_block::ObjectId;
use counter_types::{Counter as CounterModel, CounterContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

#[component]
pub fn Counter(editor: Editor) -> NodeId {
    let counter = editor.block_content::<CounterContent>();
    let count = counter.field(ObjectId::ROOT, CounterModel::COUNT);
    let shown = create_memo(move || count.get().get().to_string());
    let decrement = move || counter.operate(CounterModel::add(-1));
    let increment = move || counter.operate(CounterModel::add(1));
    let reset = move || {
        if let Ok(edit) = counter.read(|content| content.root().reset()) {
            counter.operate(edit);
        }
    };
    view! {
        <Frame padding=PADDING>
            <List spacing=16.0>
                <Text style=TextStyle::DISPLAY @test_id={"counter.value"}>{shown}</Text>
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
