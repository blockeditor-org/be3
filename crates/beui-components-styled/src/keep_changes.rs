use beui_core::layer::Dismiss;
use std::time::{Duration, Instant};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::text::{Text, TextStyle};
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_core::screens::bounds;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, ClickCallback, Direction, ForEach, Frame, IntoChild, Justify, Layer, Layers, List, Memo,
    Prop, StoredCell, component_accessibility, create_effect, create_memo, create_signal,
    create_timer, now, untrack, use_screens,
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
    #[prop(default = KEEP_CHANGES_TIMEOUT)] timeout: Prop<Duration>,
    #[prop(default = 0)] round: Prop<u64>,
    #[prop(default = "keep-changes".to_owned())] id: String,
    on_keep: ClickCallback,
    on_revert: ClickCallback,
) -> NodeId {
    let deadline = StoredCell::new(None::<Instant>);
    let (left, set_left) = create_signal(timeout.peek());
    let expire = on_revert;
    let ticking = create_timer(move || {
        let due = deadline.get()?;
        let remaining = due.saturating_duration_since(now());
        set_left.set(remaining);
        if remaining.is_zero() {
            deadline.set(None);
            expire.call();
            return None;
        }
        Some(until_next_second(remaining))
    });
    let open = create_memo(move || open.get());
    create_effect(move || {
        let opened = open.get();
        round.get();
        untrack(|| match opened {
            true => {
                let timeout = timeout.get();
                deadline.set(Some(now() + timeout));
                set_left.set(timeout);
                ticking.restart(until_next_second(timeout));
            }
            false => {
                deadline.set(None);
                ticking.stop();
            }
        });
    });
    let message = create_memo(move || {
        let seconds = left.get().as_secs_f64().ceil() as u64;
        match seconds {
            1 => "Reverting in 1 second.".to_owned(),
            seconds => format!("Reverting in {seconds} seconds."),
        }
    });
    let stop = move || {
        deadline.set(None);
        ticking.stop();
    };
    let keep = move || {
        stop();
        on_keep.call();
    };
    let revert = move || {
        stop();
        on_revert.call();
    };
    let screens = use_screens();
    let bounds = create_memo(move || screens.with(|screens| bounds(screens)));
    let places = create_memo(move || (0..screens.with(Vec::len)).collect::<Vec<usize>>());
    let anchor = create_memo(move || OverlayAnchor::Point(bounds.get().min));
    let width = create_memo(move || Some(bounds.get().width()));
    let height = create_memo(move || Some(bounds.get().height()));
    let title = create_memo(move || title.get());
    view! {
        <Overlay
            anchor={anchor}
            open={open}
            placement=Placement::At
            scrim=SCRIM
            on_dismiss={move |_: Dismiss| revert()}
            content={move || view! {
                <Frame width={width} height={height}>
                    <Layers>
                        <ForEach keys={places}>
                            {move |index: usize| {
                                let place = create_memo(move || {
                                    let origin = bounds.get().min.to_vec2();
                                    screens.with(|screens| {
                                        screens
                                            .get(index)
                                            .map_or(Rect::ZERO, |screen| screen.rect.translate(-origin))
                                    })
                                });
                                let card = view! {
                                    <KeepChangesCard
                                        place={place}
                                        title={title}
                                        message={message}
                                        focused={create_memo(move || index == 0 && open.get())}
                                        id={format!("{}.{index}", id)}
                                        on_keep={keep}
                                        on_revert={revert}
                                    />
                                };
                                let layer: Layer = card.into_child();
                                layer
                            }}
                        </ForEach>
                    </Layers>
                </Frame>
            }}
        />
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
    let left = create_memo(move || Some(place.get().left()));
    let top = create_memo(move || Some(place.get().top()));
    let width = create_memo(move || Some(place.get().width()));
    let height = create_memo(move || Some(place.get().height()));
    let label = title;
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
                        color={theme.surface_raised}
                        outline={theme.border}
                        outline_width=BORDER_WIDTH
                        outline_visible=true
                        radius=CARD_RADIUS
                        padding_horizontal=PADDING_HORIZONTAL
                        padding_vertical=PADDING_VERTICAL
                        @test_id={id.clone()}
                    >
                        <List spacing=SPACING>
                            <Text style=TextStyle::TITLE string={title} />
                            <Text
                                style=TextStyle::PARAGRAPH
                                string={message}
                                @test_id={format!("{id}.countdown")}
                                wrap=true
                            />
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
                                    on_click={on_revert}
                                />
                                <Button
                                    label="Keep"
                                    variant=ButtonVariant::Primary
                                    focused={focused}
                                    @test_id={format!("{id}.keep")}
                                    on_click={on_keep}
                                />
                            </List>
                        </List>
                    </Frame>
                </Frame>
            </Frame>
        </Frame>
    }
}
