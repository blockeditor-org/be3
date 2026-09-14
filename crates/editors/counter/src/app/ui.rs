use std::rc::Rc;

use block_client::blocks::counter::Counter;
use block_editor_plugin::beui::reactive::{
    CenteredRow, Column, Frame, ItemSize, build, clone, create_memo, view, with_reactive_scope,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Display, use_theme};
use block_editor_plugin::beui::{Color32, Context, Document, NodeId, Rect};

use super::CounterEditor;

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

pub struct CounterUi {
    document: Document,
    counter: Rc<CounterEditor>,
}

impl CounterUi {
    pub(super) fn new(counter: Rc<CounterEditor>) -> Self {
        let root = counter.clone();
        let document = build(move || {
            view! {
                <CounterView counter={root} />
            }
        });
        Self { document, counter }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn background(&self) -> Color32 {
        self.document.theme().background
    }

    pub(super) fn pump(&mut self) {
        let source = self.counter.source().clone();
        with_reactive_scope(&mut self.document, move || source.pump());
    }

    pub(super) fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
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
