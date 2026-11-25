use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use accesskit::{Live, Node, Role};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::Placement;
use beui_view::types::{LayerLevel, LayerPolicy};

use crate::floating::Edge;
use crate::gestures;
use crate::scroll::{Scroll, ScrollStyle};
use beui_view::types::{DRAG_THRESHOLD, Direction, DragGesture, PointerPress, Velocity};
use beui_view::types::{Extent, Sides, vec2};

pub const TOAST_DURATION: Duration = Duration::from_secs(8);
pub const TOAST_LIMIT: usize = 3;
/// A toast let go farther than this share of its width from where it rests goes.
pub const TOAST_SWIPE_SHARE: f32 = 0.4;
/// A toast let go faster than this, in points a second, away from where it rests goes.
pub const TOAST_SWIPE_SPEED: f32 = 500.0;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ToastAction {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub source: String,
    pub title: String,
    pub message: String,
    pub danger: bool,
    pub image: Option<Image>,
    pub actions: Vec<ToastAction>,
    pub activates: bool,
    pub sticky: bool,
}

impl Toast {
    pub fn expires(&self) -> bool {
        !self.sticky && !self.danger
    }
}

impl Hash for Toast {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        self.title.hash(state);
        self.message.hash(state);
        self.danger.hash(state);
        self.image.as_ref().map(Image::id).hash(state);
        self.actions.hash(state);
        self.activates.hash(state);
        self.sticky.hash(state);
    }
}

#[derive(Clone)]
pub struct ToastHandle {
    pub toast: Toast,
    pub dismiss: Callback<()>,
    pub activate: Callback<()>,
    pub act: Callback<String>,
}

/// The toast stack: a `Pass` overlay at `LayerLevel::Top`, so toasts stay
/// pressable over a dialog, with a live region (an error is an assertive
/// alert).
///
/// Only the newest `limit` show, with `more` drawing the count of the rest; a
/// hidden toast's timer, on the frame clock, starts once it shows. A toast that
/// is `sticky` or `danger` stays until dismissed.
///
/// A toast comes in from the side of the stack's `edge` (the right but for a
/// `*Start` edge) and goes back out that way on `spring`, and a drag across it
/// carries it: let go past `TOAST_SWIPE_SHARE` of its width or faster than
/// `TOAST_SWIPE_SPEED` away from its rest, it flies off the way it was going at
/// the speed it was let go at, and `on_dismiss` hears it once it is off;
/// otherwise it springs back from that speed. A dismiss (its handle's or its
/// time running out) sends it off from rest, or, while it is held, once it is
/// let go, at the speed it is let go at. A toast taken out of `toasts` slides
/// off the same way before it goes; one dismissed and kept collapses.
#[component]
pub fn Toasts(
    anchor: Prop<OverlayAnchor>,
    toasts: Prop<Vec<Toast>>,
    #[prop(default = Some(TOAST_DURATION))] duration: Prop<Option<Duration>>,
    edge: Prop<Edge>,
    width: Prop<f32>,
    margin: Prop<f32>,
    spacing: Prop<f32>,
    spring: Spring,
    #[prop(default = String::from("Notifications"))] label: Prop<String>,
    #[prop(default = TOAST_LIMIT)] limit: Prop<usize>,
    scroll: ScrollStyle,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    more: RenderFn<Memo<usize>, NodeId>,
    #[prop(children)] card: RenderFn<ToastHandle, NodeId>,
) -> NodeId {
    let all = create_memo(move || toasts.get());
    let hidden = create_memo(move || all.with(|toasts| toasts.len().saturating_sub(limit.get())));
    let toasts = create_memo(move || all.with(|toasts| toasts[hidden.get()..].to_vec()));
    let shown = create_signal(Vec::<Toast>::new());
    let off = store(RefCell::new(Vec::<u64>::new()));
    create_effect(move || {
        let wanted = toasts.get();
        let next =
            off.with(|off| keep_leaving(&shown.get_untracked(), wanted, &mut off.borrow_mut()));
        shown.set(next);
    });
    let open = create_memo(move || shown.with(|shown| !shown.is_empty()));
    let bottom =
        edge.map(|edge| matches!(edge, Edge::Bottom | Edge::BottomStart | Edge::BottomEnd));
    let placement = edge.map(|edge| match edge {
        Edge::Top => Placement::InsideTop,
        Edge::TopStart => Placement::InsideTopStart,
        Edge::TopEnd => Placement::InsideTopEnd,
        Edge::Bottom => Placement::InsideBottom,
        Edge::BottomStart => Placement::InsideBottomStart,
        Edge::BottomEnd => Placement::InsideBottomEnd,
    });
    let label = create_memo(move || label.get());
    let padding = create_memo(move || Sides::all(margin.get()));
    let side = create_memo(move || match edge.get() {
        Edge::TopStart | Edge::BottomStart => -1.0,
        Edge::Top | Edge::TopEnd | Edge::Bottom | Edge::BottomEnd => 1.0,
    });
    let travel = create_memo(move || width.get() + margin.get());
    view! {
        <Overlay
            anchor
            placement={placement}
            policy=LayerPolicy::Pass
            level=LayerLevel::Top
            traps_focus=false
            open={open}
        >
            <Frame width={move || Extent::at_most(width.get() + 2.0 * margin.get())}>
                <Scroll fit=true follow_end={bottom} style={scroll}>
                    <Frame padding={padding}>
                        <ToastStack
                            toasts={toasts}
                            shown={shown}
                            off={off}
                            hidden={hidden}
                            duration
                            spacing
                            spring
                            side={side}
                            travel={travel}
                            label={label}
                            on_dismiss
                            on_action
                            on_activate
                            more
                            card
                        />
                    </Frame>
                </Scroll>
            </Frame>
        </Overlay>
    }
}

/// What the stack draws: the owner's toasts, in their order, with each it took
/// away still in place until it has slid off. One already off (dismissed and
/// heard by `on_dismiss`) is dropped rather than kept.
fn keep_leaving(shown: &[Toast], wanted: Vec<Toast>, off: &mut Vec<u64>) -> Vec<Toast> {
    let mut next = wanted;
    for (index, toast) in shown.iter().enumerate() {
        if next.iter().any(|kept| kept.id == toast.id) {
            continue;
        }
        if let Some(at) = off.iter().position(|id| *id == toast.id) {
            off.remove(at);
            continue;
        }
        let at = shown[..index]
            .iter()
            .rev()
            .find_map(|before| next.iter().position(|kept| kept.id == before.id))
            .map_or(0, |at| at + 1);
        next.insert(at, toast.clone());
    }
    next
}

#[component]
fn ToastStack(
    toasts: Memo<Vec<Toast>>,
    shown: Signal<Vec<Toast>>,
    off: Stored<RefCell<Vec<u64>>>,
    hidden: Memo<usize>,
    duration: Prop<Option<Duration>>,
    spacing: Prop<f32>,
    spring: Spring,
    side: Memo<f32>,
    travel: Memo<f32>,
    label: Prop<String>,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    more: RenderFn<Memo<usize>, NodeId>,
    card: RenderFn<ToastHandle, NodeId>,
) -> NodeId {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Log);
        node.set_label(label.get());
        node.set_live(Live::Polite);
        node
    }));
    view! {
        <List spacing>
            <Show value={move || hidden.get() > 0}>|| { more.call(hidden) }</Show>
            <ForEach value={shown}>|toast: Toast| {
                let id = toast.id;
                let lasts = match toast.expires() {
                    true => duration.get_untracked(),
                    false => None,
                };
                let swipe = Swipe {
                    spring,
                    side,
                    travel,
                };
                let taken = create_memo(move || toasts.with(|toasts| !toasts.iter().any(|toast| toast.id == id)));
                // Off: one its owner took away goes; one dismissed is told and kept
                // collapsed until its owner takes it away.
                let on_off = Callback::new(move |()| match taken.get_untracked() {
                    true => shown.update(|shown| shown.retain(|toast| toast.id != id)),
                    false => {
                        off.with(|off| off.borrow_mut().push(id));
                        on_dismiss.call(id);
                    }
                });
                view! {
                    <ToastItem toast lasts swipe taken on_off on_action on_activate card />
                }
            }</ForEach>
        </List>
    }
}

#[derive(Clone, Copy)]
struct Swipe {
    spring: Spring,
    /// The side a toast comes in from and is sent off to: 1 the right, -1 the left.
    side: Memo<f32>,
    /// How far a toast moves to be off its stack.
    travel: Memo<f32>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Heading {
    Rest,
    /// Off its stack, to the side the sign says.
    Off(f32),
}

struct Held {
    heading: Heading,
    velocity: f32,
    /// Whether it has passed where it is heading since it set out.
    passed: bool,
    /// The mouse holding it.
    mouse: Option<Grip>,
    /// A finger carrying it.
    finger: bool,
    /// Dismissed, or taken away by its owner: it goes once let go, whatever way.
    dismissed: bool,
    gone: bool,
}

struct Grip {
    /// Where it was pressed, and the toast's offset then.
    from: f32,
    start: f32,
    velocity: Velocity,
    /// Whether it has dragged the toast: a press that has not (a click on the
    /// toast's buttons) leaves where it is going alone.
    moved: bool,
}

struct Motion {
    swipe: Swipe,
    offset: Signal<f32>,
    held: RefCell<Held>,
    animation: Cell<Option<Animation>>,
    gone: Signal<bool>,
    on_off: Callback<()>,
}

impl Motion {
    fn travel(&self) -> f32 {
        self.swipe.travel.get_untracked().max(1.0)
    }

    fn target(&self, heading: Heading) -> f32 {
        match heading {
            Heading::Rest => 0.0,
            Heading::Off(side) => side * self.travel(),
        }
    }

    /// Sends it toward `heading` from `velocity`; one held moves once let go.
    fn head(&self, heading: Heading, velocity: f32) {
        {
            let mut held = self.held.borrow_mut();
            if held.gone {
                return;
            }
            held.heading = heading;
            held.velocity = velocity;
            held.passed = false;
            if held.mouse.is_some() || held.finger {
                return;
            }
        }
        if let Some(animation) = self.animation.get() {
            animation.stop();
            animation.start();
        }
    }

    fn dismiss(&self) {
        let heading = {
            let mut held = self.held.borrow_mut();
            held.dismissed = true;
            held.heading
        };
        // One already on its way off keeps going.
        if heading == Heading::Rest {
            let side = self.swipe.side.get_untracked();
            self.head(Heading::Off(side), 0.0);
        }
    }

    fn recall(&self) {
        let velocity = {
            let mut held = self.held.borrow_mut();
            held.dismissed = false;
            held.velocity
        };
        self.head(Heading::Rest, velocity);
    }

    fn grab(&self) {
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
    }

    fn carry(&self, delta: f32) {
        self.offset.update(|offset| *offset += delta);
    }

    /// Let go at `velocity`: off the way it was going if it went far or fast
    /// enough, back to rest otherwise, on the spring from that speed.
    /// One dismissed while held goes whatever way it is let go.
    fn release(&self, velocity: f32) {
        let offset = self.offset.get_untracked();
        let width = self.swipe.travel.get_untracked();
        let away = match offset == 0.0 {
            true => velocity.signum(),
            false => offset.signum(),
        };
        let thrown = velocity * away > TOAST_SWIPE_SPEED;
        let far = offset.abs() > TOAST_SWIPE_SHARE * width;
        let dismissed = self.held.borrow().dismissed;
        let heading = match (
            (far || thrown) && velocity * away > -TOAST_SWIPE_SPEED,
            dismissed,
        ) {
            (true, _) => Heading::Off(away),
            (false, true) => Heading::Off(match (velocity == 0.0, offset == 0.0) {
                (false, _) => velocity.signum(),
                (true, false) => away,
                (true, true) => self.swipe.side.get_untracked(),
            }),
            (false, false) => Heading::Rest,
        };
        self.head(heading, velocity);
    }

    fn press(&self, press: PointerPress) {
        if press.touch {
            return;
        }
        self.grab();
        let offset = self.offset.get_untracked();
        self.held.borrow_mut().mouse = Some(Grip {
            from: press.pos.x,
            start: offset,
            velocity: Velocity::new(press.time, press.pos),
            moved: false,
        });
    }

    fn drag(&self, press: PointerPress) {
        let offset = {
            let mut held = self.held.borrow_mut();
            let Some(grip) = held.mouse.as_mut() else {
                return;
            };
            grip.velocity.sample(press.time, press.pos);
            // A click's jitter is no drag.
            grip.moved |= (press.pos.x - grip.from).abs() > DRAG_THRESHOLD;
            if !grip.moved {
                return;
            }
            grip.start + press.pos.x - grip.from
        };
        self.offset.set(offset);
    }

    fn let_go(&self, cancelled: bool) {
        let Some(grip) = self.held.borrow_mut().mouse.take() else {
            return;
        };
        let (finger, heading) = {
            let held = self.held.borrow();
            (held.finger, held.heading)
        };
        match (finger, cancelled, grip.moved) {
            (true, _, _) => {}
            (false, _, false) => self.head(heading, 0.0),
            (false, true, true) => self.head(Heading::Rest, 0.0),
            (false, false, true) => self.release(grip.velocity.get().x),
        }
    }

    fn finger(&self, gesture: DragGesture) {
        if gesture.started {
            self.held.borrow_mut().finger = true;
            self.grab();
        }
        self.carry(gesture.delta.x);
        if gesture.ended || gesture.cancelled {
            self.held.borrow_mut().finger = false;
            if self.held.borrow().mouse.is_some() {
                return;
            }
            match gesture.cancelled {
                true => self.head(Heading::Rest, 0.0),
                false => self.release(gesture.velocity.x),
            }
        }
    }

    fn step(&self, frame: AnimationFrame) -> bool {
        let (heading, mut velocity) = {
            let held = self.held.borrow();
            (held.heading, held.velocity)
        };
        let target = self.target(heading);
        let mut displacement = self.offset.get_untracked() - target;
        let mut passed = self.held.borrow().passed;
        self.swipe.spring.advance(
            &mut displacement,
            &mut velocity,
            &mut passed,
            frame.delta.as_secs_f32(),
        );
        self.held.borrow_mut().passed = passed;
        let offset = target + displacement;
        let off = match heading {
            Heading::Off(side) => offset * side >= self.travel(),
            Heading::Rest => false,
        };
        if off || (displacement == 0.0 && velocity == 0.0) {
            return false;
        }
        self.held.borrow_mut().velocity = velocity;
        self.offset.set(offset);
        true
    }

    fn finish(&self) {
        let heading = self.held.borrow().heading;
        self.offset.set(self.target(heading));
        self.held.borrow_mut().velocity = 0.0;
        if let Heading::Off(_) = heading {
            let gone = std::mem::replace(&mut self.held.borrow_mut().gone, true);
            if !gone {
                self.gone.set(true);
                self.on_off.call(());
            }
        }
    }
}

#[component]
fn ToastItem(
    toast: Toast,
    lasts: Option<Duration>,
    swipe: Swipe,
    /// Whether its owner took it away, which sends it off.
    taken: Memo<bool>,
    /// Once it is off.
    on_off: Callback<()>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    card: RenderFn<ToastHandle, NodeId>,
) -> NodeId {
    let id = toast.id;
    let offset = create_signal(swipe.side.get_untracked() * swipe.travel.get_untracked());
    let gone = create_signal(false);
    let motion = store(Motion {
        swipe,
        offset,
        held: RefCell::new(Held {
            heading: Heading::Rest,
            velocity: 0.0,
            passed: false,
            mouse: None,
            finger: false,
            dismissed: false,
            gone: false,
        }),
        animation: Cell::new(None),
        gone,
        on_off,
    });
    let animation = create_animation(
        move |frame| motion.value().step(frame),
        move || motion.value().finish(),
    );
    motion.with(|motion| motion.animation.set(Some(animation)));
    animation.start();
    let expiry = create_timer(move || {
        motion.value().dismiss();
        None
    });
    if let Some(lasts) = lasts {
        expiry.start(lasts);
    }
    create_effect_on(
        move || taken.get(),
        move |&taken, was| match (taken, was.copied().unwrap_or(false)) {
            (true, _) => motion.value().dismiss(),
            // Given back before it was off (folded away and shown again): it stays.
            (false, true) => motion.value().recall(),
            (false, false) => {}
        },
    );
    let mut node = match toast.danger {
        true => Node::new(Role::Alert),
        false => Node::new(Role::Status),
    };
    node.set_live(match toast.danger {
        true => Live::Assertive,
        false => Live::Polite,
    });
    node.set_label(spoken(&toast));
    component_accessibility(node);
    let activate = match toast.activates {
        true => Callback::new(move |()| on_activate.call(id)),
        false => Callback::empty(),
    };
    let handle = ToastHandle {
        toast,
        dismiss: Callback::new(move |()| motion.value().dismiss()),
        activate,
        act: Callback::new(move |key: String| on_action.call((id, key))),
    };
    let translate = create_memo(move || vec2(offset.get(), 0.0));
    let opacity = create_memo(move || {
        let travel = swipe.travel.get().max(1.0);
        1.0 - (offset.get().abs() / travel).clamp(0.0, 1.0)
    });
    view! {
        <Frame collapsed={gone}>
            <Interactive
                press_focus=false
                on_pointer={gestures::scroll_drag(
                    Some(Direction::Horizontal),
                    move |gesture: DragGesture| motion.value().finger(gesture),
                )}
                on_press={move |press: PointerPress| motion.value().press(press)}
                on_drag={move |press: PointerPress| motion.value().drag(press)}
                on_cancel={move || motion.value().let_go(true)}
                on_active_change={move |active: bool| {
                    if !active {
                        motion.value().let_go(false);
                    }
                }}
                children={Some(view! {
                    <Frame translate={translate} opacity={opacity}>{card.call(handle)}</Frame>
                })}
            />
        </Frame>
    }
}

fn spoken(toast: &Toast) -> String {
    match (toast.title.is_empty(), toast.message.is_empty()) {
        (false, false) => format!("{}: {}", toast.title, toast.message),
        (false, true) => toast.title.clone(),
        (true, _) => toast.message.clone(),
    }
}
