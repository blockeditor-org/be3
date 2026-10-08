use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::text::{Paragraph, Title};
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::{Pos2, Rect};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, ClickCallback, Direction, ForEach, Frame, IntoChild, Justify, Layer, Layers, List, Memo,
    Prop, clone, component_accessibility, create_effect, create_memo, create_signal, create_timer,
    now, untrack,
};

pub const KEEP_CHANGES_TIMEOUT: Duration = Duration::from_secs(15);

const WIDTH: f32 = 400.0;
const MARGIN: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 20.0;
const PADDING_VERTICAL: f32 = 18.0;
const SPACING: f32 = 12.0;
const BUTTON_SPACING: f32 = 8.0;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 150);
const SECOND: Duration = Duration::from_secs(1);

#[component]
pub fn KeepChanges(
    open: Prop<bool>,
    title: Prop<String>,
    #[prop(default = KEEP_CHANGES_TIMEOUT)] timeout: Duration,
    #[prop(default = Vec::new())] screens: Prop<Vec<Rect>>,
    #[prop(default = "keep-changes".to_owned())] id: String,
    on_keep: ClickCallback,
    on_revert: ClickCallback,
) -> NodeId {
    let deadline: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let (left, set_left) = create_signal(timeout);
    let settle = Rc::new(clone!(deadline -> move || deadline.take().is_some()));
    let expire = on_revert.clone();
    let ticking = create_timer(clone!(deadline settle set_left -> move || {
        let due = deadline.get()?;
        let remaining = due.saturating_duration_since(now());
        set_left.set(remaining);
        if remaining.is_zero() {
            if settle() {
                expire.call();
            }
            return None;
        }
        Some(until_next_second(remaining))
    }));
    let open = create_memo(move || open.get());
    create_effect(clone!(open deadline -> move || {
        let opened = open.get();
        untrack(|| match opened {
            true => {
                deadline.set(Some(now() + timeout));
                set_left.set(timeout);
                ticking.restart(until_next_second(timeout));
            }
            false => {
                deadline.set(None);
                ticking.stop();
            }
        });
    }));
    let message = create_memo(move || {
        let seconds = left.get().as_secs_f64().ceil() as u64;
        match seconds {
            1 => "Reverting in 1 second.".to_owned(),
            seconds => format!("Reverting in {seconds} seconds."),
        }
    });
    let keep = Rc::new(clone!(settle -> move || {
        if settle() {
            on_keep.call();
        }
    }));
    let revert = Rc::new(clone!(settle -> move || {
        if settle() {
            on_revert.call();
        }
    }));
    let dismiss = revert.clone();
    let screens = create_memo(move || screens.get());
    let places = create_memo(clone!(screens -> move || {
        let count = screens.with(Vec::len).max(1);
        (0..count).collect::<Vec<usize>>()
    }));
    let title = create_memo(move || title.get());
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open.clone()}
            placement=Placement::Fill
            scrim=SCRIM
            on_dismiss={move || dismiss()}
        >
            <Layers>
                <ForEach keys={places}>
                    {move |index: usize| {
                        let (keep, revert) = (keep.clone(), revert.clone());
                        let place = create_memo(clone!(screens -> move || {
                            screens.with(|screens| screens.get(index).copied())
                        }));
                        let card = view! {
                            <KeepChangesCard
                                place={place}
                                title={title.clone()}
                                message={message.clone()}
                                focused={create_memo(clone!(open -> move || index == 0 && open.get()))}
                                id={format!("{}.{index}", id)}
                                on_keep={move || keep()}
                                on_revert={move || revert()}
                            />
                        };
                        let layer: Layer = card.into_child();
                        layer
                    }}
                </ForEach>
            </Layers>
        </Overlay>
    }
}

fn until_next_second(remaining: Duration) -> Duration {
    match remaining.subsec_nanos() {
        0 => SECOND,
        nanos => Duration::from_nanos(u64::from(nanos)),
    }
}

#[component]
fn KeepChangesCard(
    place: Memo<Option<Rect>>,
    title: Memo<String>,
    message: Memo<String>,
    focused: Memo<bool>,
    id: String,
    on_keep: ClickCallback,
    on_revert: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let left = create_memo(clone!(place -> move || place.get().map(|rect| rect.left())));
    let top = create_memo(clone!(place -> move || place.get().map(|rect| rect.top())));
    let width = create_memo(clone!(place -> move || place.get().map(|rect| rect.width())));
    let height = create_memo(clone!(place -> move || place.get().map(|rect| rect.height())));
    let placed = create_memo(clone!(place -> move || match place.get() {
        Some(_) => Align::Start,
        None => Align::Center,
    }));
    let label = title.clone();
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::AlertDialog);
        node.set_label(label.get());
        node
    }));
    view! {
        <Frame
            padding_left={left}
            padding_top={top}
            align_horizontal={placed.clone()}
            align_vertical={placed}
        >
            <Frame
                width={width}
                height={height}
                align_horizontal=Align::Center
                align_vertical=Align::Center
            >
                <Frame padding_horizontal=MARGIN padding_vertical=MARGIN>
                    <Frame
                        max_width=Some(WIDTH)
                        color={theme.surface_raised.clone()}
                        outline={theme.border.clone()}
                        outline_width=BORDER_WIDTH
                        outline_visible=true
                        radius=CARD_RADIUS
                        padding_horizontal=PADDING_HORIZONTAL
                        padding_vertical=PADDING_VERTICAL
                        @test_id={id.clone()}
                    >
                        <List spacing=SPACING>
                            <Title content={title} />
                            <Paragraph content={message} @test_id={format!("{id}.countdown")} />
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                justify=Justify::End
                                spacing=BUTTON_SPACING
                            >
                                <Button
                                    label="Revert"
                                    variant=ButtonVariant::Secondary
                                    @test_id={format!("{id}.revert")}
                                    on_click={move || on_revert.call()}
                                />
                                <Button
                                    label="Keep"
                                    variant=ButtonVariant::Primary
                                    focused={focused}
                                    @test_id={format!("{id}.keep")}
                                    on_click={move || on_keep.call()}
                                />
                            </List>
                        </List>
                    </Frame>
                </Frame>
            </Frame>
        </Frame>
    }
}
