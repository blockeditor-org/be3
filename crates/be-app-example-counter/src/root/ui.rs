use be_sdk_beui::Editor;
use be_sdk_beui::be_block_content::ObjectId;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_type_example_counter::{Counter as CounterModel, CounterContent};
use beui::{Align, Direction};

const PADDING: Space = Space::Xl2;
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
            <List spacing=Space::Xl>
                <Text style=TextStyle::DISPLAY @test_id={"counter.value"}>{shown}</Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem length=BUTTON_WIDTH>
                        <Button
                            label="-"
                            variant=ButtonVariant::Primary
                            @test_id={"counter.decrement"}
                            on_click={decrement}
                        />
                    </ListItem>
                    <ListItem length=BUTTON_WIDTH>
                        <Button
                            label="+"
                            variant=ButtonVariant::Primary
                            @test_id={"counter.increment"}
                            on_click={increment}
                        />
                    </ListItem>
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
