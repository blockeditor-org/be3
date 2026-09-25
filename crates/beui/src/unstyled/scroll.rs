use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{Action, Node, Role};
use beui_macros::{component, view};

use super::rubber_band::{MAX_ANIMATION_STEP, MINIMUM_VELOCITY, rubber_band, spring_back, unband};
use crate::base::{Direction, ItemSize, ScrollPosition};
use crate::color::Color32;
use crate::document::Document;
use crate::input::{DragGesture, Key, KeyPress, ScrollGesture};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Children, ClickCatcher, Focusable, Frame, List, ListChild, Memo, Offset, Prop,
    ReadSignal, Render, RenderFn, clone, component_accessibility, create_memo, create_signal,
    each_frame, set_component_state, untrack, with_document,
};

const INERTIA_FRICTION: f32 = 4.5;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_INSET: f32 = -1.0;
const STEP: f32 = 40.0;

pub struct ScrollHandle {
    pub position: Memo<ScrollPosition>,
    pub direction: Prop<Direction>,
    pub scroll_to: Callback<f32>,
}

#[derive(Clone, Default)]
pub struct ScrollbarStyle(Option<(f32, RenderFn<ScrollHandle, ListChild>)>);

impl ScrollbarStyle {
    pub fn new(spacing: f32, bar: impl Fn(ScrollHandle) -> ListChild + 'static) -> Self {
        Self(Some((spacing, RenderFn::new(bar))))
    }

    fn spacing(&self) -> f32 {
        self.0.as_ref().map_or(0.0, |(spacing, _)| *spacing)
    }

    fn beside(
        &self,
        position: Memo<ScrollPosition>,
        direction: Prop<Direction>,
        scroll_to: Callback<f32>,
    ) -> Children<ListChild> {
        match &self.0 {
            None => Children::default(),
            Some((_, bar)) => Children::from(bar.call(ScrollHandle {
                position,
                direction,
                scroll_to,
            })),
        }
    }
}

struct Momentum {
    overscroll: f32,
    drag: Option<f32>,
    dragging: bool,
    velocity: f32,
    stepped: Instant,
}

impl Momentum {
    fn new() -> Self {
        Self {
            overscroll: 0.0,
            drag: None,
            dragging: false,
            velocity: 0.0,
            stepped: Instant::now(),
        }
    }

    fn moving(&self) -> bool {
        self.overscroll != 0.0 || self.velocity != 0.0
    }

    fn rest(&mut self) {
        self.overscroll = 0.0;
        self.drag = None;
        self.velocity = 0.0;
    }

    fn grab(&mut self, position: &ScrollPosition) {
        self.drag = Some(position.offset + unband(self.overscroll, position.viewport));
        self.velocity = 0.0;
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
        if self.velocity.abs() < MINIMUM_VELOCITY && self.overscroll == 0.0 {
            self.velocity = 0.0;
        }
    }

    fn animate(&mut self, position: &mut ScrollPosition, elapsed: f32, banding: bool) {
        let elapsed = elapsed.min(MAX_ANIMATION_STEP);
        if elapsed <= 0.0 {
            return;
        }
        if self.overscroll != 0.0 {
            spring_back(&mut self.overscroll, &mut self.velocity, elapsed);
            return;
        }
        if self.velocity == 0.0 {
            return;
        }
        let raw = position.offset + self.velocity * elapsed;
        position.offset = raw.clamp(0.0, position.max_offset());
        self.velocity *= (-INERTIA_FRICTION * elapsed).exp();
        if raw != position.offset && !banding {
            self.velocity = 0.0;
        } else if raw != position.offset {
            self.overscroll = raw - position.offset;
        } else if self.velocity.abs() < MINIMUM_VELOCITY {
            self.velocity = 0.0;
        }
    }
}

fn rubber_banding() -> bool {
    with_document(|document| document.rubber_banding())
}

#[derive(Clone)]
struct Motion {
    node: NodeId,
    reported: ReadSignal<Option<ScrollPosition>>,
    direction: Prop<Direction>,
    momentum: Rc<RefCell<Momentum>>,
}

impl Motion {
    fn axis(&self) -> Direction {
        untrack(|| self.direction.get())
    }

    fn placed(&self) -> Option<ScrollPosition> {
        let reported = untrack(|| self.reported.get())?;
        let offset = with_document(|document| document.offset_value(self.node));
        Some(ScrollPosition { offset, ..reported })
    }

    fn publish(&self, momentum: &Momentum, offset: f32) {
        with_document(|document| {
            document.drive_offset(self.node, offset);
            document.set_offset_overscroll(self.node, momentum.overscroll);
        });
    }

    fn wheel(&self, gesture: ScrollGesture) {
        let wheel = self.axis().main(gesture.delta);
        let Some(position) = self.placed().filter(|_| wheel != 0.0) else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        momentum.rest();
        let offset = (position.offset - wheel).clamp(0.0, position.max_offset());
        self.publish(&momentum, offset);
    }

    fn drag(&self, gesture: DragGesture) {
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
            momentum.release(-self.axis().main(gesture.velocity));
        } else if gesture.cancelled {
            momentum.release(0.0);
        }
        self.publish(&momentum, position.offset);
        if momentum.moving() {
            with_document(|document| document.request_repaint_after(Duration::ZERO));
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

    fn step(&self) {
        if !with_document(|document| document.contains(self.node)) {
            return;
        }
        let steered = with_document(|document| document.take_offset_steered(self.node));
        let mut momentum = self.momentum.borrow_mut();
        let now = Instant::now();
        let elapsed = now.duration_since(momentum.stepped).as_secs_f32();
        momentum.stepped = now;
        if steered {
            momentum.rest();
            with_document(|document| document.set_offset_overscroll(self.node, 0.0));
        }
        if std::mem::take(&mut momentum.dragging) {
            return;
        }
        momentum.drag = None;
        if !momentum.moving() {
            return;
        }
        let Some(mut position) = self.placed() else {
            return;
        };
        momentum.animate(&mut position, elapsed, rubber_banding());
        self.publish(&momentum, position.offset);
        if momentum.moving() {
            with_document(|document| document.request_repaint_after(Duration::ZERO));
        }
    }
}

#[component]
fn Scrolling(
    direction: Prop<Direction>,
    focus_color: Prop<Color32>,
    scrollbar: ScrollbarStyle,
    on_change: Callback<ScrollPosition>,
    #[prop(children)] content: Render<Callback<ScrollPosition>>,
) -> NodeId {
    let (reported, set_reported) = create_signal(None::<ScrollPosition>);
    let node = content.call(Callback::new(move |position: ScrollPosition| {
        set_reported.set(Some(position));
        on_change.call(position);
    }));
    let motion = Motion {
        node,
        reported: reported.clone(),
        direction: direction.clone(),
        momentum: Rc::new(RefCell::new(Momentum::new())),
    };
    each_frame(clone!(motion -> move || motion.step()));
    set_component_state(motion.clone());

    let position = create_memo(clone!(reported -> move || reported.get().unwrap_or_default()));
    component_accessibility(create_memo(
        clone!(reported -> move || scroll_view(reported.get())),
    ));

    let (focused, set_focused) = create_signal(false);
    let across = across(&direction);
    let axis = create_memo(clone!(direction -> move || Some(direction.get())));
    let (keyed, ancestor_keyed) = (motion.clone(), motion.clone());
    let (wheeled, dragged) = (motion.clone(), motion.clone());
    let scroll_to = Callback::new(move |offset: f32| motion.scroll_to(offset));
    view! {
        <List direction={across} spacing={scrollbar.spacing()}>
            <Focusable
                @sizing=ItemSize::Percent(100.0)
                on_focus_change={move |focused: bool| set_focused.set(focused)}
                on_key={move |press: KeyPress| keyed.key(press)}
                on_ancestor_key={move |press: KeyPress| ancestor_keyed.key(press)}
            >
                <ClickCatcher
                    scroll_axis={axis}
                    on_scroll={move |gesture: ScrollGesture| wheeled.wheel(gesture)}
                    on_scroll_drag={move |gesture: DragGesture| dragged.drag(gesture)}
                >
                    <Frame
                        outline={focus_color}
                        outline_width=FOCUS_RING_WIDTH
                        outline_offset=FOCUS_RING_INSET
                        outline_visible={focused}
                    >
                        {node}
                    </Frame>
                </ClickCatcher>
            </Focusable>
            {scrollbar.beside(position, direction, scroll_to)}
        </List>
    }
}

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Color32::TRANSPARENT)] focus_color: Prop<Color32>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let content_direction = direction.clone();
    view! {
        <Scrolling
            direction
            focus_color
            scrollbar
            on_change={move |position: ScrollPosition| on_change.call(position)}
        >
            {move |report: Callback<ScrollPosition>| {
                view! {
                    <Offset
                        offset
                        reveal
                        direction={content_direction}
                        on_change={move |position: ScrollPosition| report.call(position)}
                    >
                        {children}
                    </Offset>
                }
            }}
        </Scrolling>
    }
}

pub fn scroll_animating(document: &Document, scroll: NodeId) -> bool {
    document
        .component_state::<Motion>(scroll)
        .momentum
        .borrow()
        .moving()
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
    let direction = direction.clone();
    create_memo(move || match direction.get() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    })
}
