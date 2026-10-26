use beui_view::types::LayerPolicy;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{Action, Node, Role};
use beui_macros::{component, view};

use super::fling::Fling;
use super::rubber_band::{MINIMUM_VELOCITY, bounce, rubber_band};
use beui_core::interact::autoscroll::AUTOSCROLL_DEAD_ZONE;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::KeyAt;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::KeyPhase;
use beui_view::types::Outline;
use beui_view::types::Placement;
use beui_view::types::Pos2;
use beui_view::types::Sides;
use beui_view::types::{
    AutoscrollGesture, DragGesture, Key, KeyPress, PointerPress, ScrollGesture,
};
use beui_view::types::{Direction, ItemSize, Justify, ScrollPosition, Sizing};

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
    visibility: Signal<f32>,
    poked: Cell<Instant>,
    timer: Cell<Option<Timer>>,
    animation: Cell<Option<Animation>>,
}

impl Reveal {
    fn show(&self) {
        self.fade_to(1.0);
    }

    fn fade_to(&self, visibility: f32) {
        if self.visibility.get_untracked() != visibility {
            self.visibility.set(visibility);
        }
    }

    fn poke(&self) {
        self.poked.set(beui_view::reactive::now());
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        self.show();
        if let Some(timer) = self.timer.get() {
            timer.restart(SCROLLBAR_LINGER);
        }
    }

    fn step(&self) -> Option<Duration> {
        if self.held.get_untracked() {
            self.show();
            return None;
        }
        let elapsed = beui_view::reactive::now().duration_since(self.poked.get());
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
    stretch: f32,
    drag: Option<f32>,
    dragging: bool,
    velocity: f32,
    fling: Option<Fling>,
    autoscroll: f32,
}

impl Momentum {
    fn new() -> Self {
        Self {
            stretch: 0.0,
            drag: None,
            dragging: false,
            velocity: 0.0,
            fling: None,
            autoscroll: 0.0,
        }
    }

    fn moving(&self) -> bool {
        self.stretch != 0.0
            || self.velocity != 0.0
            || self.fling.is_some()
            || self.autoscroll != 0.0
    }

    fn flinging(&self) -> bool {
        self.fling.is_some() && self.stretch == 0.0 && self.drag.is_none()
    }

    fn coasting(&self) -> bool {
        (self.fling.is_some() || self.stretch != 0.0) && self.drag.is_none()
    }

    fn stop_fling(&mut self) {
        if self.flinging() {
            self.fling = None;
            self.velocity = 0.0;
        }
    }

    fn rest(&mut self) {
        self.stretch = 0.0;
        self.drag = None;
        self.velocity = 0.0;
        self.fling = None;
        self.autoscroll = 0.0;
    }

    fn grab(&mut self, position: &ScrollPosition) {
        self.drag = Some(position.offset + self.stretch);
        self.velocity = 0.0;
        self.fling = None;
    }

    fn drag(&mut self, position: &mut ScrollPosition, delta: f32, banding: bool) {
        let raw = self.drag.unwrap_or(position.offset) - delta;
        position.offset = raw.clamp(0.0, position.max_offset());
        self.drag = Some(if banding { raw } else { position.offset });
        self.stretch = match banding {
            true => raw - position.offset,
            false => 0.0,
        };
        self.velocity = 0.0;
    }

    fn release(&mut self, velocity: f32) {
        self.drag = None;
        self.velocity = velocity;
        if self.stretch == 0.0 {
            self.coast();
        }
    }

    fn coast(&mut self) {
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
        if self.stretch != 0.0 {
            let back = bounce(&mut self.stretch, &mut self.velocity, elapsed);
            if self.stretch == 0.0 {
                self.velocity = back;
                self.coast();
            }
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
                true => self.stretch = raw - position.offset,
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
    motion().animates()
}

struct Motion {
    node: Option<NodeId>,
    reported: ReadSignal<Option<ScrollPosition>>,
    direction: Prop<Direction>,
    momentum: RefCell<Momentum>,
    animation: Cell<Option<Animation>>,
    autoscrolling: Cell<Option<Timer>>,
    autoscrolled: Cell<Instant>,
}

impl Motion {
    fn axis(&self) -> Direction {
        self.direction.get_untracked()
    }

    fn placed(&self) -> Option<ScrollPosition> {
        let reported = self.reported.get_untracked()?;
        let node = self.node?;
        let offset = node.offset()?;
        let position = ScrollPosition { offset, ..reported };
        Some(ScrollPosition {
            offset: position.clamped(),
            ..position
        })
    }

    fn publish(&self, momentum: &Momentum, position: &ScrollPosition, offset: f32) {
        let Some(node) = self.node else {
            return;
        };
        node.drive_offset(offset + rubber_band(momentum.stretch, position.viewport));
    }

    fn settle(&self, position: ScrollPosition) -> bool {
        let Some(node) = self.node else {
            return false;
        };
        let Ok(momentum) = self.momentum.try_borrow() else {
            return false;
        };
        let stray = position.overscroll() != 0.0 && momentum.drag.is_none() && !momentum.moving();
        if stray {
            node.drive_offset(position.clamped());
        }
        stray
    }

    fn coasting(&self) -> bool {
        self.momentum.borrow().coasting()
    }

    fn stop_fling(&self) {
        self.momentum.borrow_mut().stop_fling();
    }

    fn wheel(&self, gesture: ScrollGesture) {
        let wheel = self.axis().main(gesture.delta);
        let fling = self.axis().main(gesture.fling);
        let Some(mut position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        if !gesture.held && momentum.drag.is_none() && wheel == 0.0 && fling == 0.0 {
            return;
        }
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        if gesture.held && rubber_banding() {
            if momentum.drag.is_none() {
                momentum.grab(&position);
            }
            momentum.drag(&mut position, wheel, true);
            self.publish(&momentum, &position, position.offset);
            return;
        }
        if momentum.drag.is_some() && !momentum.dragging {
            momentum.release(-fling);
            self.publish(&momentum, &position, position.offset);
            self.animate(&mut momentum);
            return;
        }
        if wheel == 0.0 && fling == 0.0 {
            return;
        }
        momentum.rest();
        let offset = (position.offset - wheel).clamp(0.0, position.max_offset());
        self.publish(&momentum, &position, offset);
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
        self.publish(&momentum, &position, position.offset);
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
        if momentum.stretch != 0.0
            && let Some(position) = self.placed()
        {
            momentum.rest();
            self.publish(&momentum, &position, position.offset);
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
        self.publish(
            &momentum,
            &position,
            offset.clamp(0.0, position.max_offset()),
        );
    }

    fn to_end(&self) {
        let Some(node) = self.node else {
            return;
        };
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        self.momentum.borrow_mut().rest();
        node.offset_to_end();
    }

    fn key(&self, press: KeyPress) -> bool {
        if press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let Some(position) = self.placed().filter(|position| position.max_offset() > 0.0) else {
            return false;
        };
        let vertical = self.axis() == Direction::Vertical;
        let (forwards, backwards) = match vertical {
            false => (Key::ArrowRight, Key::ArrowLeft),
            true => (Key::ArrowDown, Key::ArrowUp),
        };
        let offset = match press.key {
            key if key == forwards => position.offset + STEP,
            key if key == backwards => position.offset - STEP,
            Key::PageDown | Key::Space if vertical && !press.modifiers.shift => {
                position.offset + position.viewport
            }
            Key::PageUp | Key::Space if vertical => position.offset - position.viewport,
            Key::Home => 0.0,
            Key::End => position.max_offset(),
            _ => return false,
        }
        .clamp(0.0, position.max_offset());
        if offset == position.offset {
            return false;
        }
        if press.pressed {
            let mut momentum = self.momentum.borrow_mut();
            momentum.rest();
            self.publish(&momentum, &position, offset);
        }
        true
    }

    fn steer(&self) {
        if let Ok(mut momentum) = self.momentum.try_borrow_mut() {
            momentum.rest();
        }
    }

    fn finish(&self) {
        let Some(position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        let autoscroll = momentum.autoscroll;
        momentum.rest();
        momentum.autoscroll = autoscroll;
        self.publish(&momentum, &position, position.offset);
    }

    fn step(&self, elapsed: f32) -> Option<Duration> {
        let node = self.node?;
        if !node.exists() {
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
        self.publish(&momentum, &position, position.offset);
        momentum.moving().then_some(Duration::ZERO)
    }
}

#[component]
fn Scrolling(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ScrollApi>,
    direction: Prop<Direction>,
    fit: bool,
    bleed: Prop<Sides>,
    focus_ring: Prop<Option<Outline>>,
    scrollbar: ScrollbarStyle,
    marker: Render<Memo<Direction>>,
    on_change: Callback<ScrollPosition>,
    #[prop(children)] content: Render<Callback<ScrollPosition>>,
) -> NodeId {
    let reported = create_signal(None::<ScrollPosition>);
    let settles = Rc::new(Cell::new(None::<Stored<Motion>>));
    let node = content.call(Callback::new({
        let settles = Rc::clone(&settles);
        move |position: ScrollPosition| {
            if settles
                .get()
                .is_some_and(|motion| motion.value().settle(position))
            {
                return;
            }
            reported.set(Some(position));
            on_change.call(position);
        }
    }));
    let motion = store(Motion {
        node: node.first_offset_within(),
        reported: reported.read_only(),
        direction,
        momentum: RefCell::new(Momentum::new()),
        animation: Cell::default(),
        autoscrolling: Cell::default(),
        autoscrolled: Cell::new(now()),
    });
    settles.set(Some(motion));
    let animation = create_animation(
        move |frame| motion.value().step(frame.delta.as_secs_f32()).is_some(),
        move || motion.value().finish(),
    );
    let autoscrolling = create_timer(move || {
        let state = motion.value();
        if state.momentum.borrow().autoscroll == 0.0 {
            return None;
        }
        let elapsed = now().saturating_duration_since(state.autoscrolled.replace(now()));
        state.step(elapsed.as_secs_f32())
    });
    motion.with(|motion| motion.autoscrolling.set(Some(autoscrolling)));
    motion.with(|motion| motion.animation.set(Some(animation)));
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    create_effect(move || {
        if !animates.get() {
            motion.value().finish();
        }
    });
    if let Some(offset) = motion.with(|motion| motion.node) {
        offset.on_steer(move |()| motion.value().steer());
    }
    api.fill(ScrollApi(motion));

    let position = create_memo(move || reported.get().unwrap_or_default());
    let overflows = create_memo(move || position.get().max_offset() > 0.0);
    component_accessibility(create_memo(move || {
        scroll_view(reported.get(), direction.get())
    }));

    let near = create_signal(false);
    let engaged = create_signal(false);
    let reveal = store(Reveal {
        held: create_memo(move || near.get() || engaged.get()),
        visibility: create_signal(0.0),
        poked: Cell::new(beui_view::reactive::now()),
        timer: Cell::default(),
        animation: Cell::default(),
    });
    let animation = create_animation(
        move |_| reveal.value().step().is_some(),
        move || {
            if !reveal.value().held.get_untracked() {
                reveal.value().fade_to(0.0);
            }
        },
    );
    reveal.with(|reveal| reveal.animation.set(Some(animation)));
    let fading = create_timer(move || {
        if !reveal.value().held.get_untracked() {
            animation.start();
            if animation.running() && reveal.value().step().is_none() {
                animation.finish();
            }
        }
        None
    });
    reveal.with(|reveal| reveal.timer.set(Some(fading)));
    let moved = Cell::new(None::<f32>);
    create_effect(move || {
        let Some(position) = reported.get() else {
            return;
        };
        let at = position.offset;
        if moved.replace(Some(at)).is_some_and(|was| was != at) {
            reveal.value().poke();
        }
    });
    create_effect(move || {
        let held = reveal.with(|reveal| reveal.held.get());
        match held {
            true => {
                reveal.value().show();
                fading.stop();
                animation.stop();
            }
            false => {
                if reveal.with(|reveal| reveal.visibility.get_untracked()) > 0.0 {
                    reveal.value().poke();
                }
            }
        }
    });
    let bar_width = scrollbar.width();
    let bar = ScrollHandle {
        position,
        direction,
        scroll_to: Callback::new(move |offset: f32| motion.value().scroll_to(offset)),
        visibility: reveal.with(|reveal| reveal.visibility.read_only()),
        on_engage: Callback::new(move |now: bool| {
            if engaged.get_untracked() != now {
                engaged.set(now);
            }
        }),
    };

    let focused = create_signal(false);
    let across = across(&direction);
    let axis = create_memo(move || Some(direction.get()));
    let catcher = NodeRef::new();
    let origin = create_signal(None::<Pos2>);
    let marked = create_memo(move || origin.get().is_some());
    let anchor = create_memo(move || OverlayAnchor::Point(origin.get().unwrap_or(Pos2::ZERO)));
    let marker_axis = create_memo(move || direction.get());
    let marker = marker.once();
    on_scroll_key(KeyAt::Node(catcher), KeyPhase::Bubble, move |event| {
        event
            .press()
            .is_some_and(|press| navigates(press.key) && motion.value().key(press))
    });
    let outset = create_memo(move || -bleed.get());
    view! {
        <Frame padding={outset}>
            <Layers>
                <Interactive
                    focusable=true
                    tab_stop={overflows}
                    yields_tab_stop=true
                    on_focus_change={focused}
                    on_key={move |press: KeyPress| motion.value().key(press)}
                    scroll_axis={axis}
                    @node_ref=&catcher
                    claim_at={move |(pos, _): (Pos2, beui_view::types::Modifiers)| {
                        motion.value().coasting() && under(&catcher, pos)
                    }}
                    on_press={move |_: PointerPress| motion.value().stop_fling()}
                    on_hover_move={move |press: PointerPress| {
                        let at_edge = bar_width > 0.0
                            && position.get_untracked().max_offset() > 0.0
                            && near_edge(direction.get_untracked(), press, bar_width);
                        if near.get_untracked() != at_edge {
                            near.set(at_edge);
                        }
                    }}
                    on_hover_change={move |inside: bool| {
                        if !inside && near.get_untracked() {
                            near.set(false);
                        }
                    }}
                    on_scroll={move |gesture: ScrollGesture| motion.value().wheel(gesture)}
                    on_scroll_drag={move |gesture: DragGesture| motion.value().drag(gesture)}
                    on_autoscroll={move |gesture: AutoscrollGesture| {
                        let marking = (!gesture.ended).then_some(gesture.origin);
                        if origin.get_untracked() != marking {
                            origin.set(marking);
                        }
                        motion.value().autoscroll(gesture);
                    }}
                >
                    <Frame outline={ring_when(focused, focus_ring)}>
                        <List direction={marker_axis} spacing=0.0>
                            {node} @sizing={content_sizing(fit)}
                            <Overlay
                                anchor
                                placement=Placement::Around
                                policy=LayerPolicy::Inert
                                traps_focus=false
                                open={marked}
                                keep_alive=true
                            >
                                {marker.call(marker_axis)}
                            </Overlay>
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
    #[prop(default = ComponentRef::new())] api: ComponentRef<ScrollApi>,
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    #[prop(default = false)] follow_end: Prop<bool>,
    #[prop(default = Sides::ZERO)] bleed: Prop<Sides>,
    #[prop(default = None)] focus_ring: Prop<Option<Outline>>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    marker: Option<Render<Memo<Direction>>>,
    on_change: Callback<ScrollPosition>,
    children: Children<ListChild>,
) -> NodeId {
    let fade = scrollbar.fade();
    let faded = create_signal(None::<ScrollPosition>);
    let edges = create_memo(move || fade_edges(direction.get(), faded.get(), fade));
    let rows = NodeRef::new();
    let marker = marker.unwrap_or_else(|| {
        Render::new(|_| {
            view! {
                <List spacing=0.0 />
            }
        })
    });
    let scroll = view! {
        <Scrolling
            api
            direction
            fit
            bleed
            focus_ring
            scrollbar
            marker
            on_change={on_change}
        >|report: Callback<ScrollPosition>|
            <Fade edges>
                <Offset
                    offset
                    direction={direction}
                    fit
                    follow_end
                    on_change={move |position: ScrollPosition| {
                        faded.set(Some(position));
                        report.call(position);
                    }}
                >
                    <Frame padding={bleed}>
                        <List @node_ref=&rows direction={direction} spacing=0.0>{children}</List>
                    </Frame>
                </Offset>
            </Fade>
        </Scrolling>
    };
    create_effect(move || {
        let Some(index) = reveal.get() else {
            return;
        };
        let rows = rows.get();
        if let Some(row) = rows.children().get(index).copied() {
            row.reveal();
        }
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

fn scroll_view(position: Option<ScrollPosition>, direction: Direction) -> Node {
    let mut node = Node::new(Role::ScrollView);
    if let Some(position) = position {
        let (offset, max) = (position.clamped().into(), position.max_offset().into());
        match direction {
            Direction::Horizontal => {
                node.set_scroll_x(offset);
                node.set_scroll_x_min(0.0);
                node.set_scroll_x_max(max);
                node.add_action(Action::ScrollLeft);
                node.add_action(Action::ScrollRight);
            }
            Direction::Vertical => {
                node.set_scroll_y(offset);
                node.set_scroll_y_min(0.0);
                node.set_scroll_y_max(max);
                node.add_action(Action::ScrollUp);
                node.add_action(Action::ScrollDown);
            }
        }
        node.add_action(Action::Focus);
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
        placements()
            .rect(node)
            .is_some_and(|rect| rect.contains(pos))
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

#[derive(Clone, Copy)]
pub struct ScrollApi(Stored<Motion>);

impl ScrollApi {
    pub fn animating(&self) -> bool {
        self.0.with(|motion| motion.momentum.borrow().moving())
    }

    pub fn scroll_to_end(&self) {
        self.0.with(|motion| motion.to_end());
    }
}

fn ring_when(focused: impl IntoProp<bool>, ring: Prop<Option<Outline>>) -> Memo<Option<Outline>> {
    let shown = focus_ring(focused);
    create_memo(move || ring.get().filter(|_| shown.get()))
}
