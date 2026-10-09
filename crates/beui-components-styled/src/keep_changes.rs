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
use beui_core::geometry::Rect;
use beui_core::screens::bounds;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, ClickCallback, Direction, ForEach, Frame, IntoChild, Justify, Layer, Layers, List, Memo,
    Prop, clone, component_accessibility, create_effect, create_memo, create_signal, create_timer,
    now, untrack, use_screens,
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
    #[prop(default = "keep-changes".to_owned())] id: String,
    on_keep: ClickCallback,
    on_revert: ClickCallback,
) -> NodeId {
    let deadline: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let (left, set_left) = create_signal(timeout);
    let expire = on_revert.clone();
    let ticking = create_timer(clone!(deadline set_left -> move || {
        let due = deadline.get()?;
        let remaining = due.saturating_duration_since(now());
        set_left.set(remaining);
        if remaining.is_zero() {
            deadline.set(None);
            expire.call();
            return None;
        }
        Some(until_next_second(remaining))
    }));
    let open = create_memo(move || open.get());
    create_effect(clone!(open deadline ticking -> move || {
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
    let stop = Rc::new(move || {
        deadline.set(None);
        ticking.stop();
    });
    let keep = Rc::new(clone!(stop -> move || {
        stop();
        on_keep.call();
    }));
    let revert = Rc::new(move || {
        stop();
        on_revert.call();
    });
    let dismiss = revert.clone();
    let screens = use_screens();
    let bounds = create_memo(clone!(screens -> move || screens.with(|screens| bounds(screens))));
    let places = create_memo(clone!(screens -> move || {
        (0..screens.with(Vec::len)).collect::<Vec<usize>>()
    }));
    let anchor = create_memo(clone!(bounds -> move || OverlayAnchor::Point(bounds.get().min)));
    let width = create_memo(clone!(bounds -> move || Some(bounds.get().width())));
    let height = create_memo(clone!(bounds -> move || Some(bounds.get().height())));
    let title = create_memo(move || title.get());
    view! {
        <Overlay
            anchor={anchor}
            open={open.clone()}
            placement=Placement::At
            scrim=SCRIM
            on_dismiss={move || dismiss()}
        >
            <Frame width={width} height={height}>
                <Layers>
                    <ForEach keys={places}>
                        {move |index: usize| {
                            let (keep, revert) = (keep.clone(), revert.clone());
                            let place = create_memo(clone!(screens bounds -> move || {
                                let origin = bounds.get().min.to_vec2();
                                screens.with(|screens| {
                                    screens
                                        .get(index)
                                        .map_or(Rect::ZERO, |screen| screen.rect.translate(-origin))
                                })
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
            </Frame>
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
    place: Memo<Rect>,
    title: Memo<String>,
    message: Memo<String>,
    focused: Memo<bool>,
    id: String,
    on_keep: ClickCallback,
    on_revert: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let left = create_memo(clone!(place -> move || Some(place.get().left())));
    let top = create_memo(clone!(place -> move || Some(place.get().top())));
    let width = create_memo(clone!(place -> move || Some(place.get().width())));
    let height = create_memo(clone!(place -> move || Some(place.get().height())));
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
            align_horizontal=Align::Start
            align_vertical=Align::Start
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
