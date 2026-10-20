use beui_core::layer::LayerPolicy;
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use accesskit::{Action, Node, Role};
use beui_macros::{component, view};

use super::fling::Fling;
use super::rubber_band::{MINIMUM_VELOCITY, SCROLL_SPRING, rubber_band, spring_back, unband};
use beui_core::base::Sides;
use beui_core::base::offset::OffsetNode;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::base::{Direction, ItemSize, Justify, ScrollPosition, Sizing};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::Pos2;
use beui_core::input::{
    AutoscrollGesture, DragGesture, Key, KeyPress, PointerPress, ScrollGesture,
};
use beui_core::interact::autoscroll::AUTOSCROLL_DEAD_ZONE;
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::{NodeId, NodeOf};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_INSET: f32 = -1.0;
const STEP: f32 = 40.0;
const AUTOSCROLL_GAIN: f32 = 6.0;
const AUTOSCROLL_ACCELERATION: f32 = 0.02;

pub const SCROLLBAR_LINGER: Duration = Duration::from_millis(1000);
pub const SCROLLBAR_FADE_OUT: Duration = Duration::from_millis(250);

pub struct ScrollHandle {
    pub position: Memo<ScrollPosition>,
    pub direction: Prop<Direction>,
    pub scroll_to: Callback<f32>,
    pub visibility: ReadSignal<f32>,
    pub on_engage: Callback<bool>,
}

#[derive(Clone, Copy, Default)]
pub struct ScrollbarStyle {
    bar: Option<(f32, RenderFn<ScrollHandle, NodeId>)>,
    fade: f32,
}

impl ScrollbarStyle {
    pub fn new(width: f32, bar: impl Fn(ScrollHandle) -> NodeId + 'static) -> Self {
        Self {
            bar: Some((width, RenderFn::new(bar))),
            fade: 0.0,
        }
    }

    pub fn fading(self, fade: f32) -> Self {
        Self { fade, ..self }
    }

    pub fn fade(&self) -> f32 {
        self.fade
    }

    fn width(&self) -> f32 {
        self.bar.as_ref().map_or(0.0, |(width, _)| *width)
    }

    fn over(&self, handle: ScrollHandle) -> Children<ListChild> {
        match &self.bar {
            None => Children::default(),
            Some((width, bar)) => {
                Children::from(with_sizing(bar.call(handle), ItemSize::Fixed(*width)))
            }
        }
    }
}

struct Reveal {
    held: Memo<bool>,
    visibility: (ReadSignal<f32>, WriteSignal<f32>),
    poked: Cell<Instant>,
    timer: Cell<Option<Timer>>,
    animation: Cell<Option<Animation>>,
}

impl Reveal {
    fn show(&self) {
        self.fade_to(1.0);
    }

    fn fade_to(&self, visibility: f32) {
        let (shown, set_shown) = self.visibility;
        if untrack(|| shown.get()) != visibility {
            set_shown.set(visibility);
        }
    }

    fn poke(&self) {
        self.poked.set(beui_core::timer::now());
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        self.show();
        if let Some(timer) = self.timer.get() {
            timer.restart(SCROLLBAR_LINGER);
        }
    }

    fn step(&self) -> Option<Duration> {
        if untrack(|| self.held.get()) {
            self.show();
            return None;
        }
        let elapsed = beui_core::timer::now().duration_since(self.poked.get());
        if elapsed < SCROLLBAR_LINGER {
            self.show();
            return Some(SCROLLBAR_LINGER - elapsed);
        }
        let fading = (elapsed - SCROLLBAR_LINGER).as_secs_f32() / SCROLLBAR_FADE_OUT.as_secs_f32();
        let visibility = (1.0 - fading).max(0.0);
        self.fade_to(visibility);
        (visibility > 0.0).then_some(Duration::ZERO)
    }
}

struct Momentum {
    overscroll: f32,
    drag: Option<f32>,
    dragging: bool,
    velocity: f32,
    fling: Option<Fling>,
    autoscroll: f32,
}

impl Momentum {
    fn new() -> Self {
        Self {
            overscroll: 0.0,
            drag: None,
            dragging: false,
            velocity: 0.0,
            fling: None,
            autoscroll: 0.0,
        }
    }

    fn moving(&self) -> bool {
        self.overscroll != 0.0
            || self.velocity != 0.0
            || self.fling.is_some()
            || self.autoscroll != 0.0
    }

    fn flinging(&self) -> bool {
        self.fling.is_some() && self.overscroll == 0.0 && self.drag.is_none()
    }

    fn stop_fling(&mut self) {
        if self.flinging() {
            self.fling = None;
            self.velocity = 0.0;
        }
    }

    fn rest(&mut self) {
        self.overscroll = 0.0;
        self.drag = None;
        self.velocity = 0.0;
        self.fling = None;
        self.autoscroll = 0.0;
    }

    fn grab(&mut self, position: &ScrollPosition) {
        self.drag = Some(position.offset + unband(self.overscroll, position.viewport));
        self.velocity = 0.0;
        self.fling = None;
    }

    fn drag(&mut self, position: &mut ScrollPosition, delta: f32, banding: bool) {
        let raw = self.drag.unwrap_or(position.offset) - delta;
        position.offset = raw.clamp(0.0, position.max_offset());
        self.drag = Some(if banding { raw } else { position.offset });
        self.overscroll = match banding {
            true => rubber_band(raw - position.offset, position.viewport),
            false => 0.0,
        };
        self.velocity = 0.0;
    }

    fn release(&mut self, velocity: f32) {
        self.drag = None;
        self.velocity = if self.overscroll == 0.0 {
            velocity
        } else {
            velocity * 0.35
        };
        if self.overscroll != 0.0 {
            return;
        }
        self.fling = (self.velocity.abs() >= MINIMUM_VELOCITY)
            .then(|| Fling::new(self.velocity))
            .flatten();
        self.velocity = self.fling.map_or(0.0, |fling| fling.velocity());
    }

    fn animate(&mut self, position: &mut ScrollPosition, elapsed: f32, banding: bool) {
        if elapsed <= 0.0 {
            return;
        }
        if self.autoscroll != 0.0 {
            let raw = position.offset + self.autoscroll * elapsed;
            position.offset = raw.clamp(0.0, position.max_offset());
            if raw != position.offset {
                self.autoscroll = 0.0;
            }
            return;
        }
        if self.overscroll != 0.0 {
            spring_back(
                &mut self.overscroll,
                &mut self.velocity,
                elapsed,
                SCROLL_SPRING,
            );
            return;
        }
        let Some(fling) = &mut self.fling else {
            self.velocity = 0.0;
            return;
        };
        let raw = position.offset + fling.advance(elapsed);
        self.velocity = fling.velocity();
        if fling.done() {
            self.fling = None;
        }
        position.offset = raw.clamp(0.0, position.max_offset());
        if raw != position.offset {
            self.fling = None;
            match banding {
                true => self.overscroll = raw - position.offset,
                false => self.velocity = 0.0,
            }
        }
    }
}

fn autoscroll_speed(distance: f32) -> f32 {
    let beyond = (distance.abs() - AUTOSCROLL_DEAD_ZONE).max(0.0);
    distance.signum() * (beyond * AUTOSCROLL_GAIN + beyond * beyond * AUTOSCROLL_ACCELERATION)
}

fn rubber_banding() -> bool {
    with_document(|document| document.motion().animates())
}

struct Motion {
    node: Option<NodeOf<OffsetNode>>,
    reported: ReadSignal<Option<ScrollPosition>>,
    direction: Prop<Direction>,
    momentum: RefCell<Momentum>,
    animation: Cell<Option<Animation>>,
    autoscrolling: Cell<Option<Timer>>,
    autoscrolled: Cell<Instant>,
}

impl Motion {
    fn axis(&self) -> Direction {
        untrack(|| self.direction.get())
    }

    fn placed(&self) -> Option<ScrollPosition> {
        let reported = untrack(|| self.reported.get())?;
        let node = self.node?;
        let offset = with_document(|document| document.offset_value(node));
        Some(ScrollPosition { offset, ..reported })
    }

    fn publish(&self, momentum: &Momentum, offset: f32) {
        let Some(node) = self.node else {
            return;
        };
        with_document(|document| {
            document.drive_offset(node, offset);
            document.set_offset_overscroll(node, momentum.overscroll);
        });
    }

    fn flinging(&self) -> bool {
        self.momentum.borrow().flinging()
    }

    fn stop_fling(&self) {
        self.momentum.borrow_mut().stop_fling();
    }

    fn wheel(&self, gesture: ScrollGesture) {
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        let wheel = self.axis().main(gesture.delta);
        let fling = self.axis().main(gesture.fling);
        let Some(position) = self.placed().filter(|_| wheel != 0.0 || fling != 0.0) else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        momentum.rest();
        let offset = (position.offset - wheel).clamp(0.0, position.max_offset());
        self.publish(&momentum, offset);
        if fling != 0.0 && rubber_banding() {
            momentum.release(-fling);
            self.animate(&mut momentum);
        }
    }

    fn drag(&self, gesture: DragGesture) {
        if gesture.started
            && let Some(animation) = self.animation.get()
        {
            animation.stop();
        }
        let Some(mut position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        momentum.dragging = !gesture.ended && !gesture.cancelled;
        if momentum.drag.is_none() {
            momentum.grab(&position);
        }
        let dragged = self.axis().main(gesture.delta);
        if dragged != 0.0 {
            momentum.drag(&mut position, dragged, rubber_banding());
        }
        if gesture.ended {
            momentum.release(if rubber_banding() {
                -self.axis().main(gesture.velocity)
            } else {
                0.0
            });
        } else if gesture.cancelled {
            momentum.release(0.0);
        }
        self.publish(&momentum, position.offset);
        if gesture.ended || gesture.cancelled {
            self.animate(&mut momentum);
        }
    }

    fn autoscroll(&self, gesture: AutoscrollGesture) {
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        let mut momentum = self.momentum.borrow_mut();
        if gesture.ended {
            momentum.autoscroll = 0.0;
            return;
        }
        if momentum.overscroll != 0.0
            && let Some(position) = self.placed()
        {
            momentum.rest();
            self.publish(&momentum, position.offset);
        }
        momentum.drag = None;
        momentum.velocity = 0.0;
        momentum.fling = None;
        momentum.autoscroll = autoscroll_speed(self.axis().main(gesture.pos - gesture.origin));
        drop(momentum);
        self.autoscrolled.set(now());
        self.autoscrolling.get().unwrap().start(Duration::ZERO);
    }

    fn animate(&self, momentum: &mut Momentum) {
        let Some(animation) = self.animation.get() else {
            return;
        };
        if rubber_banding() && momentum.moving() {
            animation.start();
        }
    }

    fn scroll_to(&self, offset: f32) {
        let Some(position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        momentum.rest();
        self.publish(&momentum, offset.clamp(0.0, position.max_offset()));
    }

    fn key(&self, press: KeyPress) -> bool {
        if press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let Some(position) = self.placed() else {
            return false;
        };
        let (forwards, backwards) = match self.axis() {
            Direction::Horizontal => (Key::ArrowRight, Key::ArrowLeft),
            Direction::Vertical => (Key::ArrowDown, Key::ArrowUp),
        };
        let offset = match press.key {
            key if key == forwards => position.offset + STEP,
            key if key == backwards => position.offset - STEP,
            Key::PageDown | Key::Space if !press.modifiers.shift => {
                position.offset + position.viewport
            }
            Key::PageUp | Key::Space => position.offset - position.viewport,
            Key::Home => 0.0,
            Key::End => position.max_offset(),
            _ => return false,
        };
        if press.pressed {
            let mut momentum = self.momentum.borrow_mut();
            momentum.rest();
            self.publish(&momentum, offset.clamp(0.0, position.max_offset()));
        }
        true
    }

    fn steer(&self) {
        let Some(node) = self.node else {
            return;
        };
        if let Ok(mut momentum) = self.momentum.try_borrow_mut() {
            momentum.rest();
        }
        with_document(|document| {
            if document.contains(node) {
                document.set_offset_overscroll(node, 0.0);
            }
        });
    }

    fn finish(&self) {
        let Some(position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        let autoscroll = momentum.autoscroll;
        momentum.rest();
        momentum.autoscroll = autoscroll;
        self.publish(&momentum, position.offset);
    }

    fn step(&self, elapsed: f32) -> Option<Duration> {
        let node = self.node?;
        if !with_document(|document| document.contains(node)) {
            return None;
        }
        let mut momentum = self.momentum.borrow_mut();
        if std::mem::take(&mut momentum.dragging) {
            return Some(Duration::ZERO);
        }
        momentum.drag = None;
        if !momentum.moving() {
            return None;
        }
        let mut position = self.placed()?;
        momentum.animate(&mut position, elapsed, rubber_banding());
        self.publish(&momentum, position.offset);
        momentum.moving().then_some(Duration::ZERO)
    }
}

#[component]
fn Scrolling(
    direction: Prop<Direction>,
    fit: bool,
    bleed: Prop<Sides>,
    tab_stop: bool,
    focus_color: Prop<Color32>,
    scrollbar: ScrollbarStyle,
    marker: Render<Memo<Direction>>,
    on_change: Callback<ScrollPosition>,
    #[prop(children)] content: Render<Callback<ScrollPosition>>,
) -> NodeId {
    let (reported, set_reported) = create_signal(None::<ScrollPosition>);
    let node = content.call(Callback::new(move |position: ScrollPosition| {
        set_reported.set(Some(position));
        on_change.call(position);
    }));
    let motion = store(Motion {
        node: with_document(|document| document.first_offset_within(node)),
        reported,
        direction,
        momentum: RefCell::new(Momentum::new()),
        animation: Cell::default(),
        autoscrolling: Cell::default(),
        autoscrolled: Cell::new(now()),
    });
    let animation = create_animation(
        move |frame| motion.get().step(frame.delta.as_secs_f32()).is_some(),
        move || motion.get().finish(),
    );
    let autoscrolling = create_timer(move || {
        let state = motion.get();
        if state.momentum.borrow().autoscroll == 0.0 {
            return None;
        }
        let elapsed = now().saturating_duration_since(state.autoscrolled.replace(now()));
        state.step(elapsed.as_secs_f32())
    });
    motion.with(|motion| motion.autoscrolling.set(Some(autoscrolling)));
    motion.with(|motion| motion.animation.set(Some(animation)));
    let preference = with_document(|document| document.watch_motion());
    create_effect(move || {
        if !preference.get().animates() {
            motion.get().finish();
        }
    });
    if let Some(offset) = motion.with(|motion| motion.node) {
        with_document(|document| {
            document.set_offset_on_steer(offset, move |()| motion.get().steer());
        });
    }
    set_component_state(motion);

    let position = create_memo(move || reported.get().unwrap_or_default());
    component_accessibility(create_memo(move || scroll_view(reported.get())));

    let (near, set_near) = create_signal(false);
    let (engaged, set_engaged) = create_signal(false);
    let reveal = store(Reveal {
        held: create_memo(move || near.get() || engaged.get()),
        visibility: create_signal(0.0),
        poked: Cell::new(beui_core::timer::now()),
        timer: Cell::default(),
        animation: Cell::default(),
    });
    let animation = create_animation(
        move |_| reveal.get().step().is_some(),
        move || {
            if !reveal.get().held.get_untracked() {
                reveal.get().fade_to(0.0);
            }
        },
    );
    reveal.with(|reveal| reveal.animation.set(Some(animation)));
    let fading = create_timer(move || {
        if !reveal.get().held.get_untracked() {
            animation.start();
            if animation.running() && reveal.get().step().is_none() {
                animation.finish();
            }
        }
        None
    });
    reveal.with(|reveal| reveal.timer.set(Some(fading)));
    let moved = Cell::new(None::<(f32, f32)>);
    create_effect(move || {
        let Some(position) = reported.get() else {
            return;
        };
        let at = (position.offset, position.overscroll);
        if moved.replace(Some(at)).is_some_and(|was| was != at) {
            reveal.get().poke();
        }
    });
    create_effect(move || {
        let held = reveal.with(|reveal| reveal.held.get());
        match held {
            true => {
                reveal.get().show();
                fading.stop();
                animation.stop();
            }
            false => {
                if untrack(|| reveal.with(|reveal| reveal.visibility.0.get())) > 0.0 {
                    reveal.get().poke();
                }
            }
        }
    });
    let bar_width = scrollbar.width();
    let edge_axis = direction;
    let bar = ScrollHandle {
        position,
        direction,
        scroll_to: Callback::new(move |offset: f32| motion.get().scroll_to(offset)),
        visibility: reveal.with(|reveal| reveal.visibility.0),
        on_engage: Callback::new(move |now: bool| {
            if untrack(|| engaged.get()) != now {
                set_engaged.set(now);
            }
        }),
    };

    let (focused, set_focused) = create_signal(false);
    let across = across(&direction);
    let axis = create_memo(move || Some(direction.get()));
    let catcher = NodeRef::new();
    let catcher_rect = catcher;
    let (origin, set_origin) = create_signal(None::<Pos2>);
    let marked = create_memo(move || origin.get().is_some());
    let anchor = create_memo(move || OverlayAnchor::Point(origin.get().unwrap_or(Pos2::ZERO)));
    let marker_axis = create_memo(move || direction.get());
    let content_axis = marker_axis;
    on_scroll_key(KeyAt::Node(catcher), KeyPhase::Bubble, move |event| {
        event
            .press()
            .is_some_and(|press| navigates(press.key) && motion.get().key(press))
    });
    let outset = create_memo(move || -bleed.get());
    view! {
        <Frame padding={outset}>
            <Layers>
                <Interactive
                    focusable=true
                    tab_stop
                    on_focus_change={move |focused: bool| set_focused.set(focused)}
                    on_key={move |press: KeyPress| motion.get().key(press)}
                    scroll_axis={axis}
                    @node_ref=&catcher
                    claim_at={move |(pos, _): (Pos2, beui_core::input::Modifiers)| {
                        motion.get().flinging() && under(&catcher_rect, pos)
                    }}
                    on_press={move |_: PointerPress| motion.get().stop_fling()}
                    on_hover_move={move |press: PointerPress| {
                        let at_edge = bar_width > 0.0
                            && untrack(|| position.get()).max_offset() > 0.0
                            && near_edge(untrack(|| edge_axis.get()), press, bar_width);
                        if untrack(|| near.get()) != at_edge {
                            set_near.set(at_edge);
                        }
                    }}
                    on_hover_change={move |inside: bool| {
                        if !inside && untrack(|| near.get()) {
                            set_near.set(false);
                        }
                    }}
                    on_scroll={move |gesture: ScrollGesture| motion.get().wheel(gesture)}
                    on_scroll_drag={move |gesture: DragGesture| motion.get().drag(gesture)}
                    on_autoscroll={move |gesture: AutoscrollGesture| {
                        let marking = (!gesture.ended).then_some(gesture.origin);
                        if untrack(|| origin.get()) != marking {
                            set_origin.set(marking);
                        }
                        motion.get().autoscroll(gesture);
                    }}
                >
                    <Frame
                        outline={focus_outline(focused, focus_color, FOCUS_RING_WIDTH, FOCUS_RING_INSET)}
                    >
                        <List direction={content_axis} spacing=0.0>
                            {node} @sizing={content_sizing(fit)}
                            <ShowKeepAlive condition={marked}>
                                <Overlay
                                    anchor
                                    placement=Placement::Around
                                    policy=LayerPolicy::Inert
                                    traps_focus=false
                                    open={marked}
                                >
                                    {marker.call(marker_axis)}
                                </Overlay>
                            </ShowKeepAlive>
                        </List>
                    </Frame>
                </Interactive>
                <List direction={across} justify=Justify::End spacing=0.0>
                    {scrollbar.over(bar)}
                </List>
            </Layers>
        </Frame>
    }
}

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    #[prop(default = Sides::ZERO)] bleed: Prop<Sides>,
    #[prop(default = true)] tab_stop: bool,
    #[prop(default = Color32::TRANSPARENT)] focus_color: Prop<Color32>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    marker: Option<Render<Memo<Direction>>>,
    on_change: Callback<ScrollPosition>,
    children: Children<ListChild>,
) -> NodeId {
    let content_direction = direction;
    let rows_direction = direction;
    let fade = scrollbar.fade();
    let (faded, set_faded) = create_signal(None::<ScrollPosition>);
    let edges = create_memo(move || fade_edges(direction.get(), faded.get(), fade));
    let rows = NodeRef::new();
    let revealed = rows;
    let marker = marker.unwrap_or_else(|| {
        Render::new(|_| {
            view! {
                <List spacing=0.0 />
            }
        })
    });
    let scroll = view! {
        <Scrolling
            direction
            fit
            bleed
            tab_stop
            focus_color
            scrollbar
            marker
            on_change={on_change}
        >|report: Callback<ScrollPosition>|
            <Fade edges>
                <Offset
                    offset
                    direction={content_direction}
                    fit
                    on_change={move |position: ScrollPosition| {
                        set_faded.set(Some(position));
                        report.call(position);
                    }}
                >
                    <Frame padding={bleed}>
                        <List @node_ref=&rows direction={rows_direction} spacing=0.0>
                            {children}
                        </List>
                    </Frame>
                </Offset>
            </Fade>
        </Scrolling>
    };
    create_effect(move || {
        let Some(index) = reveal.get() else {
            return;
        };
        let rows = revealed.get();
        with_document(|document| {
            let row = document.arena.get(rows).children().get(index).copied();
            if let Some(row) = row {
                document.reveal_node(row);
            }
        });
    });
    scroll
}

fn content_sizing(fit: bool) -> Sizing {
    match fit {
        true => ItemSize::Intrinsic.shrink(1.0),
        false => ItemSize::Percent(100.0).into(),
    }
}

pub(crate) fn fade_edges(
    direction: Direction,
    position: Option<ScrollPosition>,
    fade: f32,
) -> Sides {
    let Some(position) = position.filter(|_| fade > 0.0) else {
        return Sides::default();
    };
    let reach = fade.min(position.viewport / 2.0);
    let before = position.offset.clamp(0.0, reach);
    let after = (position.max_offset() - position.offset).clamp(0.0, reach);
    match direction {
        Direction::Horizontal => Sides {
            left: before,
            top: 0.0,
            right: after,
            bottom: 0.0,
        },
        Direction::Vertical => Sides {
            left: 0.0,
            top: before,
            right: 0.0,
            bottom: after,
        },
    }
}

pub fn scroll_animating(document: &Document, scroll: NodeId) -> bool {
    document
        .component_state::<Stored<Motion>>(scroll)
        .with(|motion| motion.momentum.borrow().moving())
}

fn scroll_view(position: Option<ScrollPosition>) -> Node {
    let mut node = Node::new(Role::ScrollView);
    if let Some(position) = position {
        node.set_scroll_y(position.offset.into());
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max(position.max_offset().into());
        node.add_action(Action::Focus);
        node.add_action(Action::ScrollUp);
        node.add_action(Action::ScrollDown);
        node.add_action(Action::SetScrollOffset);
    }
    node
}

fn across(direction: &Prop<Direction>) -> Memo<Direction> {
    let direction = *direction;
    create_memo(move || match direction.get() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    })
}

pub(crate) fn under(node: &NodeRef, pos: Pos2) -> bool {
    node.try_get().is_some_and(|node| {
        with_document(|document| document.node_rect(node)).is_some_and(|rect| rect.contains(pos))
    })
}

fn near_edge(direction: Direction, press: PointerPress, width: f32) -> bool {
    let across = match direction {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    };
    !press.touch && across.main(press.size) - across.main(press.local.to_vec2()) <= width
}

fn navigates(key: Key) -> bool {
    matches!(
        key,
        Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
    )
}
