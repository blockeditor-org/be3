use std::rc::Rc;

use block_client::blocks::counter::Counter;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    CenteredRow, Column, Frame, ItemSize, clone, create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Display, use_theme};

use super::CounterEditor;

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

pub struct CounterUi;

impl CounterUi {
    pub(super) fn new(counter: Rc<CounterEditor>) -> (Self, NodeId) {
        let root = view! {
            <CounterView counter={counter} />
        };
        (Self, root)
    }
}

#[block_editor_plugin::beui::reactive::component]
fn CounterView(counter: Rc<CounterEditor>) -> NodeId {
    let count = counter.source().project(Counter::count);
    let shown = create_memo(clone!(count -> move || count.get().to_string()));
    let decrement = counter.clone();
    let increment = counter.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <Column spacing=16.0>
                <Display content={shown} @test_id={"counter.value"} />
                <CenteredRow spacing=10.0>
                    <Button
                        @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                        label="-"
                        variant=ButtonVariant::Primary
                        @test_id={"counter.decrement"}
                        on_click={move || decrement.decrement()}
                    />
                    <Button
                        @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                        label="+"
                        variant=ButtonVariant::Primary
                        @test_id={"counter.increment"}
                        on_click={move || increment.increment()}
                    />
                    <Button
                        label="Reset"
                        variant=ButtonVariant::Secondary
                        @test_id={"counter.reset"}
                        on_click={move || counter.reset()}
                    />
                </CenteredRow>
            </Column>
        </Frame>
    }
}
