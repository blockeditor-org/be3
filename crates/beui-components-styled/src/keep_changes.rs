use crate::dialog::{DialogActions, DialogSurface};
use crate::frame::Frame;
use crate::theme::use_theme;
use beui_core::layer::Dismiss;
use std::time::{Duration, Instant};

use accesskit::Role;
use beui_macros::{component, view};

use crate::text::{Text, TextStyle};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_core::screens::bounds;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub const KEEP_CHANGES_TIMEOUT: Duration = Duration::from_secs(15);

const WIDTH: f32 = 400.0;
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
    let left = create_signal(timeout.get_untracked());
    let ticking = create_timer(move || {
        let due = deadline.get()?;
        let remaining = due.saturating_duration_since(now());
        left.set(remaining);
        if remaining.is_zero() {
            deadline.set(None);
            on_revert.call();
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
                left.set(timeout);
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
    let theme = use_theme();
    let scrim = create_memo(move || theme.scrim.get());
    view! {
        <Overlay
            anchor={anchor}
            open={open}
            placement=Placement::At
            scrim={scrim}
            on_dismiss={move |_: Dismiss| revert()}
            content={move || { let id = id.clone(); view! {
                <Frame width=width height=height>
                    <Layers>
                        <ForEach keys={places}>|index: usize| {
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
                        }</ForEach>
                    </Layers>
                </Frame>
            } }}
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
    let inset = create_memo(move || Sides::new(place.get().left(), place.get().top(), 0.0, 0.0));
    let width = create_memo(move || place.get().width());
    let height = create_memo(move || place.get().height());
    let actions_id = id.clone();
    let actions = DialogActions {
        id: create_memo(move || actions_id.clone()),
        confirm: create_memo(|| "Keep".to_owned()),
        cancel: create_memo(|| "Revert".to_owned()),
        destructive: create_memo(|| false),
        confirm_disabled: create_memo(|| false),
        focus_confirm: focused,
        on_confirm: on_keep,
        on_cancel: on_revert,
    };
    view! {
        <Frame padding=inset align=Align::Start>
            <Frame width=width height=height align=Align::Center>
                <DialogSurface
                    width=WIDTH
                    title={title}
                    role=Role::AlertDialog
                    actions={actions}
                    surface_id={id.clone()}
                >
                    <Text style=TextStyle::PARAGRAPH @test_id={format!("{id}.countdown")}>
                        {message}
                    </Text>
                </DialogSurface>
            </Frame>
        </Frame>
    }
}
