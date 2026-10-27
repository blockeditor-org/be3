use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui_tree::reactive::Callback;
use beui_types::{Direction, ScrollPosition};

use crate::node::NodeId;

const END_SLACK: f32 = 1.0;

thread_local! {
    static SCROLLERS: RefCell<HashMap<NodeId, Weak<Scroller>>> = RefCell::default();
}

pub struct Scroller {
    node: NodeId,
    child: NodeId,
    direction: Cell<Direction>,
    offset: Cell<f32>,
    follows: Cell<bool>,
    pinned: Cell<bool>,
    to_end: Cell<bool>,
    anchored: Cell<bool>,
    reveal: Cell<Option<NodeId>>,
    position: Cell<Option<ScrollPosition>>,
    reported: Cell<Option<ScrollPosition>>,
    on_change: Callback<ScrollPosition>,
    on_steer: Cell<Callback<()>>,
    #[cfg(target_arch = "wasm32")]
    web: RefCell<web::Held>,
}

pub fn scroller(node: NodeId, child: NodeId, on_change: Callback<ScrollPosition>) -> Rc<Scroller> {
    let scroller = Rc::new(Scroller {
        node,
        child,
        direction: Cell::new(Direction::Vertical),
        offset: Cell::new(0.0),
        follows: Cell::new(false),
        pinned: Cell::new(true),
        to_end: Cell::new(false),
        anchored: Cell::new(false),
        reveal: Cell::new(None),
        position: Cell::new(None),
        reported: Cell::new(None),
        on_change,
        on_steer: Cell::new(Callback::empty()),
        #[cfg(target_arch = "wasm32")]
        web: RefCell::default(),
    });
    SCROLLERS.with(|held| {
        let mut held = held.borrow_mut();
        held.retain(|_, scroller| scroller.strong_count() > 0);
        held.insert(node, Rc::downgrade(&scroller));
    });
    let pass = Rc::clone(&scroller);
    crate::frame::on_layout(Rc::new(move || pass.lay_out()));
    scroller
}

pub fn find(node: NodeId) -> Option<Rc<Scroller>> {
    SCROLLERS.with(|held| held.borrow().get(&node).and_then(Weak::upgrade))
}

pub fn first_within(node: NodeId) -> Option<NodeId> {
    if find(node).is_some() {
        return Some(node);
    }
    let root = crate::document::element(node)?;
    let scrollers: Vec<(NodeId, beui_html::Node)> = SCROLLERS.with(|held| {
        held.borrow()
            .values()
            .filter_map(Weak::upgrade)
            .filter_map(|scroller| Some((scroller.node, crate::document::element(scroller.node)?)))
            .collect()
    });
    let found =
        root.find(&|candidate| scrollers.iter().any(|(_, element)| element.is(candidate)))?;
    scrollers
        .into_iter()
        .find(|(_, element)| element.is(&found))
        .map(|(node, _)| node)
}

pub fn reveal(node: NodeId) {
    #[cfg(target_arch = "wasm32")]
    {
        let mut held = false;
        for viewer in crate::dom::nodes_above(crate::dom::dom(node)) {
            if viewer == node {
                continue;
            }
            if let Some(scroller) = find(viewer) {
                scroller.reveal.set(Some(node));
                held = true;
            }
            crate::canvas::reveal_in(viewer, node);
        }
        if !held {
            reveal_in_page(node);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = node;
    crate::frame::request_frame();
}

#[cfg(target_arch = "wasm32")]
fn reveal_in_page(node: NodeId) {
    use wasm_bindgen::JsCast;
    let Some(element) = crate::dom::element(node) else {
        return;
    };
    let options = js_sys::Object::new();
    for key in ["block", "inline"] {
        let _ = js_sys::Reflect::set(&options, &key.into(), &"nearest".into());
    }
    if let Ok(scroll) = js_sys::Reflect::get(&element, &"scrollIntoView".into())
        && let Some(scroll) = scroll.dyn_ref::<js_sys::Function>()
    {
        let _ = scroll.call1(&element, &options);
    }
}

pub fn revealed_offset(start: f32, end: f32, offset: f32, length: f32) -> Option<f32> {
    if start <= offset && end >= offset + length {
        return None;
    }
    let revealed = if start < offset {
        start
    } else if end > offset + length {
        (end - length).min(start)
    } else {
        return None;
    };
    Some(revealed.max(0.0))
}

impl Scroller {
    pub fn node(&self) -> NodeId {
        self.node
    }

    pub fn set_direction(&self, direction: Direction) {
        if self.direction.replace(direction) != direction {
            self.anchored.set(false);
            self.position.set(None);
        }
        crate::frame::request_frame();
    }

    fn moved_to(&self, value: f32) {
        self.offset.set(value);
        self.anchored.set(false);
        self.to_end.set(false);
        self.pinned.set(
            self.position
                .get()
                .is_none_or(|position| value >= position.max_offset() - END_SLACK),
        );
        crate::frame::request_frame();
    }

    pub fn set_offset(&self, value: f32) {
        if self.offset.get() != value || self.anchored.get() {
            self.moved_to(value);
        }
        self.steer();
    }

    pub fn drive(&self, value: f32) {
        if self.offset.get() != value {
            self.moved_to(value);
        }
    }

    pub fn set_follows_end(&self, follows: bool) {
        self.follows.set(follows);
        crate::frame::request_frame();
    }

    pub fn to_end(&self) {
        self.pinned.set(true);
        self.to_end.set(true);
        self.anchored.set(false);
        crate::frame::request_frame();
    }

    pub fn offset(&self) -> f32 {
        self.offset.get()
    }

    pub fn position(&self) -> Option<ScrollPosition> {
        self.position.get()
    }

    pub fn set_on_steer(&self, on_steer: Callback<()>) {
        self.on_steer.set(on_steer);
    }

    fn steer(&self) {
        let on_steer = self.on_steer.get();
        if !on_steer.is_empty() {
            on_steer.call(());
        }
    }

    fn snaps_to_end(&self) -> bool {
        self.pinned.get() && (self.follows.get() || self.to_end.get())
    }

    #[cfg(target_arch = "wasm32")]
    fn settle(&self, position: ScrollPosition) -> bool {
        let previous = self.position.replace(Some(position));
        if self.to_end.get() && previous == Some(position) {
            self.to_end.set(false);
        }
        if self.reported.replace(Some(position)) == Some(position) {
            return previous != Some(position);
        }
        if !self.on_change.is_empty() {
            self.on_change.call(position);
        }
        true
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn lay_out(&self) -> bool {
        let _ = (self.child, self.reveal.get(), self.snaps_to_end());
        let _ = (&self.reported, &self.on_change);
        false
    }

    #[cfg(target_arch = "wasm32")]
    fn lay_out(&self) -> bool {
        web::lay_out(self)
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use beui_types::{Direction, Rect, ScrollPosition};

    use super::{END_SLACK, Scroller, revealed_offset};
    use crate::dom::{Listener, Resizes};

    const ANCHOR_DEPTH: usize = 16;

    #[derive(Default)]
    pub(super) struct Held {
        element: Option<web_sys::Element>,
        resizes: Option<Resizes>,
        listener: Option<Listener>,
        applied: Option<f32>,
        anchor: Option<(web_sys::Element, f32)>,
    }

    #[derive(Clone, Copy)]
    struct Axis {
        horizontal: bool,
        origin: Rect,
        scroll: f32,
        scale: f32,
    }

    impl Axis {
        fn of(element: &web_sys::Element, direction: Direction) -> Self {
            let horizontal = direction == Direction::Horizontal;
            let origin = crate::dom::client_rect(element);
            let scroll = match horizontal {
                true => element.scroll_left() as f32,
                false => element.scroll_top() as f32,
            };
            Self {
                horizontal,
                origin,
                scroll,
                scale: crate::dom::scale_of(element, origin),
            }
        }

        fn span(&self, element: &web_sys::Element) -> (f32, f32) {
            let rect = crate::dom::client_rect(element);
            let (start, origin, length) = match self.horizontal {
                true => (rect.left(), self.origin.left(), rect.width()),
                false => (rect.top(), self.origin.top(), rect.height()),
            };
            let start = (start - origin) / self.scale + self.scroll;
            (start, start + length / self.scale)
        }
    }

    fn choose_anchor(
        child: &web_sys::Element,
        axis: &Axis,
        offset: f32,
    ) -> (web_sys::Element, f32) {
        let mut anchor = child.clone();
        for _ in 0..ANCHOR_DEPTH {
            let mut next = anchor.first_element_child();
            let mut found = None;
            while let Some(candidate) = next {
                next = candidate.next_element_sibling();
                if candidate.has_attribute("data-beui-spacer") {
                    continue;
                }
                if axis.span(&candidate).1 > offset {
                    found = Some(candidate);
                    break;
                }
            }
            match found {
                Some(found) => anchor = found,
                None => break,
            }
        }
        let start = axis.span(&anchor).0;
        (anchor, start)
    }

    fn attach(scroller: &Scroller, held: &mut Held, element: &web_sys::Element) {
        if held.element.as_ref() == Some(element) {
            return;
        }
        let resizes = Resizes::new(crate::frame::relayout);
        if let Some(resizes) = &resizes {
            resizes.observe(element);
            if let Some(child) = crate::dom::element(scroller.child) {
                resizes.observe(&child);
            }
        }
        held.resizes = resizes;
        held.listener = Some(crate::dom::listen(
            element.as_ref(),
            "scroll",
            false,
            |_: web_sys::Event| crate::frame::request_frame(),
        ));
        held.element = Some(element.clone());
        held.applied = None;
        held.anchor = None;
    }

    fn write_scroll(element: &web_sys::Element, horizontal: bool, value: f32) -> f32 {
        match horizontal {
            true => {
                element.set_scroll_left(value.round() as i32);
                element.scroll_left() as f32
            }
            false => {
                element.set_scroll_top(value.round() as i32);
                element.scroll_top() as f32
            }
        }
    }

    pub(super) fn lay_out(scroller: &Scroller) -> bool {
        let Some(element) = crate::dom::element(scroller.node) else {
            return false;
        };
        if !element.is_connected() {
            return false;
        }
        let mut held = scroller.web.borrow_mut();
        attach(scroller, &mut held, &element);
        let direction = scroller.direction.get();
        let horizontal = direction == Direction::Horizontal;
        let (content, viewport) = match horizontal {
            true => (element.scroll_width(), element.client_width()),
            false => (element.scroll_height(), element.client_height()),
        };
        let (content, viewport) = (content as f32, viewport as f32);
        let max = (content - viewport).max(0.0);
        let axis = Axis::of(&element, direction);
        let mut steered = false;
        if let Some(applied) = held.applied
            && (axis.scroll - applied.clamp(0.0, max)).abs() >= 1.0
        {
            scroller.offset.set(axis.scroll);
            scroller.anchored.set(false);
            scroller.to_end.set(false);
            scroller.pinned.set(axis.scroll >= max - END_SLACK);
            steered = true;
        }
        let snapping = scroller.snaps_to_end();
        if snapping {
            scroller.offset.set(max);
        } else if scroller.anchored.get()
            && let Some((anchor, start)) = held.anchor.as_ref()
            && anchor.is_connected()
            && element.contains(Some(anchor.as_ref()))
        {
            let drift = axis.span(anchor).0 - start;
            if drift.abs() >= 0.5 {
                scroller.offset.set(scroller.offset.get() + drift);
            }
        }
        if let Some(target) = scroller.reveal.take()
            && let Some(target) = crate::dom::element(target)
            && element.contains(Some(target.as_ref()))
        {
            let (start, end) = axis.span(&target);
            if let Some(revealed) = revealed_offset(start, end, scroller.offset.get(), viewport) {
                scroller.offset.set(revealed);
                if snapping {
                    scroller.to_end.set(false);
                    scroller.pinned.set(revealed >= max - END_SLACK);
                }
                steered = true;
            }
        }
        let wanted = scroller.offset.get().clamp(0.0, max);
        let moved = (axis.scroll - wanted).abs() >= 0.5;
        let shown = match moved {
            true => write_scroll(&element, horizontal, wanted),
            false => axis.scroll,
        };
        held.applied = Some(shown);
        let axis = Axis {
            scroll: shown,
            ..axis
        };
        held.anchor = crate::dom::element(scroller.child)
            .map(|child| choose_anchor(&child, &axis, scroller.offset.get()));
        scroller.anchored.set(true);
        drop(held);
        if steered {
            scroller.steer();
        }
        let position = ScrollPosition {
            offset: scroller.offset.get(),
            content,
            viewport,
        };
        scroller.settle(position) || moved
    }
}
