use std::cell::Cell;
use std::rc::Rc;

use beui::NodeId;
use beui::icons::ICON_CHEVRON_RIGHT;
use beui::reactive::{
    Align, ClickCallback, Direction, Dynamic, ForEach, Frame, ItemSize, List, Prop, Show, Text,
    WriteSignal, clone, component, create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Icon, Separator, use_theme};
use beui::{Context, Document, Key};

const BAND_PADDING_H: f32 = 12.0;
const BAND_PADDING_V: f32 = 8.0;
const STEP_SPACING: f32 = 6.0;

pub struct BeuiFrame {
    document: Document,
    content: NodeId,
    set_trail: WriteSignal<Vec<String>>,
    set_shown: WriteSignal<bool>,
    exit: Rc<Cell<bool>>,
}

impl BeuiFrame {
    pub fn build(view: impl FnOnce() -> NodeId) -> Self {
        let (trail, set_trail) = create_signal(Vec::<String>::new());
        let (shown, set_shown) = create_signal(false);
        let exit = Rc::new(Cell::new(false));
        let exit_writer = exit.clone();
        let content_slot: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
        let content_slot_writer = content_slot.clone();
        let document = beui::reactive::build(move || {
            let content = view();
            content_slot_writer.set(Some(content));
            view! {
                <List spacing=0.0>
                    <TopBar trail shown on_exit={move || exit_writer.set(true)} />
                    {content} @sizing=ItemSize::Percent(100.0)
                </List>
            }
        });
        Self {
            document,
            content: content_slot
                .get()
                .expect("view() builds its root node synchronously"),
            set_trail,
            set_shown,
            exit,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub fn content(&self) -> NodeId {
        self.content
    }

    pub fn set_trail(&self) -> WriteSignal<Vec<String>> {
        self.set_trail.clone()
    }

    pub fn set_shown(&self) -> WriteSignal<bool> {
        self.set_shown.clone()
    }

    pub fn exit(&self) -> Rc<Cell<bool>> {
        self.exit.clone()
    }
}

#[component]
pub(crate) fn TopBar(
    trail: Prop<Vec<String>>,
    shown: Prop<bool>,
    on_exit: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let trail = create_memo(move || trail.get());
    let visible = create_memo(clone!(trail -> move || shown.get() && !trail.get().is_empty()));
    view! {
        <Frame visible={visible} color={theme.surface.clone()}>
            <List spacing=0.0>
                <Frame padding_horizontal=BAND_PADDING_H padding_vertical=BAND_PADDING_V>
                    <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                        <List
                            @sizing=ItemSize::Percent(100.0)
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=STEP_SPACING
                        >
                            <Dynamic value={trail}>
                                {move |trail: Vec<String>| view! {
                                    <Breadcrumb trail />
                                }}
                            </Dynamic>
                        </List>
                        <Button
                            label="Close"
                            variant=ButtonVariant::Secondary
                            on_click={move || on_exit.call()}
                            @test_id={"editor.close"}
                        />
                    </List>
                </Frame>
                <Separator />
            </List>
        </Frame>
    }
}

#[component]
fn Breadcrumb(trail: Vec<String>) -> NodeId {
    let steps: Rc<Vec<String>> = Rc::new(trail);
    let last = steps.len().saturating_sub(1);
    let keys: Vec<usize> = (0..steps.len()).collect();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=STEP_SPACING>
            <ForEach keys>
                {move |index: usize| view! {
                    <Step step={steps[index].clone()} separated={index > 0} last={index == last} />
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn Step(step: String, separated: bool, last: bool) -> NodeId {
    let theme = use_theme();
    let color = match last {
        true => theme.text.clone(),
        false => theme.text_muted.clone(),
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=STEP_SPACING>
            <Show condition={separated}>
                {move || view! {
                    <Icon glyph={ICON_CHEVRON_RIGHT.to_owned()} color={theme.text_muted.clone()} />
                }}
            </Show>
            <Text string={step} color={color} />
        </List>
    }
}

pub(crate) fn escaped(context: &Context) -> bool {
    context.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                beui::Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
            )
        })
    })
}
