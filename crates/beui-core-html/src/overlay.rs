use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui_tree::reactive::Callback;
use beui_types::{Color, Dismiss, LayerLevel, LayerPolicy, Placement, Rect};

use crate::css::px;
use crate::document::{set_attribute, set_style, style_value};
use crate::node::{NodeId, NodeRef};

pub use beui_types::layer::OverlayAnchor;

const PLACED: [&str; 14] = [
    "position-anchor",
    "position-area",
    "position-try-fallbacks",
    "top",
    "left",
    "right",
    "bottom",
    "translate",
    "margin-top",
    "margin-bottom",
    "width",
    "height",
    "max-width",
    "max-height",
];

thread_local! {
    static LAYERS: RefCell<HashMap<NodeId, Weak<Layer>>> = RefCell::default();
    static OPEN: RefCell<Vec<Weak<Layer>>> = RefCell::default();
    static NEXT: Cell<u32> = const { Cell::new(0) };
}

pub struct Parts {
    pub host: NodeId,
    pub dialog: NodeId,
    pub proxy: NodeId,
    pub beak: NodeId,
}

pub struct Layer {
    parts: Parts,
    name: String,
    anchor: RefCell<OverlayAnchor>,
    placement: Cell<Placement>,
    policy: Cell<LayerPolicy>,
    level: Cell<LayerLevel>,
    traps: Cell<bool>,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    reach: Cell<f32>,
    wanted: Cell<bool>,
    on_dismiss: Callback<Dismiss>,
    #[cfg(target_arch = "wasm32")]
    web: RefCell<web::Held>,
}

pub fn layer(
    parts: Parts,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    on_dismiss: Callback<Dismiss>,
) -> Rc<Layer> {
    let index = NEXT.with(|next| next.replace(next.get() + 1));
    set_attribute(parts.dialog, "class", Some("beui-overlay".to_owned()));
    set_style(parts.host, "display", Some("contents".to_owned()));
    for (property, value) in [
        ("position", "fixed"),
        ("opacity", "0"),
        ("pointer-events", "none"),
    ] {
        set_style(parts.proxy, property, Some(value.to_owned()));
    }
    set_attribute(parts.beak, "class", Some("beui-beak".to_owned()));
    set_attribute(parts.beak, "hidden", Some(String::new()));
    let layer = Rc::new(Layer {
        name: format!("--beui-overlay-{index}"),
        parts,
        anchor: RefCell::default(),
        placement: Cell::new(Placement::BelowStart),
        policy: Cell::new(LayerPolicy::BlocksAll),
        level: Cell::new(LayerLevel::Normal),
        traps: Cell::new(true),
        trigger,
        pointer,
        reach: Cell::new(0.0),
        wanted: Cell::new(false),
        on_dismiss,
        #[cfg(target_arch = "wasm32")]
        web: RefCell::default(),
    });
    LAYERS.with(|held| {
        let mut held = held.borrow_mut();
        held.retain(|_, layer| layer.strong_count() > 0);
        held.insert(layer.parts.host, Rc::downgrade(&layer));
    });
    let pass = Rc::clone(&layer);
    crate::frame::on_layout(Rc::new(move || pass.lay_out()));
    layer
}

pub fn find(node: NodeId) -> Option<Rc<Layer>> {
    LAYERS.with(|held| held.borrow().get(&node).and_then(Weak::upgrade))
}

#[cfg(target_arch = "wasm32")]
fn open_layers() -> Vec<Rc<Layer>> {
    OPEN.with(|open| {
        let mut open = open.borrow_mut();
        open.retain(|layer| layer.upgrade().is_some_and(|layer| layer.wanted.get()));
        open.iter().filter_map(Weak::upgrade).collect()
    })
}

#[cfg(target_arch = "wasm32")]
pub fn stacked_layers() -> Vec<(NodeId, Rect)> {
    let mut open = open_layers();
    open.sort_by_key(|layer| layer.level.get());
    open.iter()
        .filter(|layer| layer.policy.get() != LayerPolicy::Inert)
        .filter_map(|layer| {
            Some((
                layer.parts.dialog,
                crate::measure::rect(layer.parts.dialog)?,
            ))
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn stacked_layers() -> Vec<(NodeId, Rect)> {
    Vec::new()
}

fn anchor_name(node: NodeId) -> String {
    format!("--beui-anchor-{}-{}", node.index(), node.generation())
}

impl Layer {
    pub fn host(&self) -> NodeId {
        self.parts.host
    }

    pub fn is_open(&self) -> bool {
        self.wanted.get()
    }

    fn modal(&self) -> bool {
        self.policy.get() == LayerPolicy::BlocksAll
    }

    pub fn set_anchor(&self, anchor: OverlayAnchor) {
        *self.anchor.borrow_mut() = anchor;
        self.place();
    }

    pub fn set_placement(&self, placement: Placement) {
        self.placement.set(placement);
        self.place();
    }

    pub fn set_scrim(&self, scrim: Color) {
        set_style(
            self.parts.dialog,
            "--beui-scrim",
            (!scrim.is_clear()).then(|| crate::css::color(scrim)),
        );
    }

    pub fn set_policy(&self, policy: LayerPolicy) {
        self.policy.set(policy);
        set_style(
            self.parts.dialog,
            "pointer-events",
            (policy == LayerPolicy::Inert).then(|| "none".to_owned()),
        );
        self.sync_attributes();
        crate::frame::request_frame();
    }

    pub fn set_level(&self, level: LayerLevel) {
        self.level.set(level);
        crate::frame::request_frame();
    }

    pub fn set_traps_focus(&self, traps: bool) {
        self.traps.set(traps);
    }

    pub fn set_beak(&self, reach: f32) {
        self.reach.set(reach.max(0.0));
        self.place_beak();
    }

    pub fn show(&self, shown: bool) {
        if self.wanted.replace(shown) == shown {
            return;
        }
        if shown {
            let weak = self.weak();
            OPEN.with(|open| open.borrow_mut().push(weak));
        }
        self.sync_attributes();
        self.place();
        crate::frame::request_frame();
    }

    fn weak(&self) -> Weak<Layer> {
        find(self.parts.host).map_or_else(Weak::new, |layer| Rc::downgrade(&layer))
    }

    pub fn raise(&self) {
        let weak = self.weak();
        OPEN.with(|open| {
            let mut open = open.borrow_mut();
            open.retain(|layer| !std::ptr::eq(layer.as_ptr(), self));
            open.push(weak);
        });
        #[cfg(target_arch = "wasm32")]
        {
            self.web.borrow_mut().shown = None;
        }
        crate::frame::request_frame();
    }

    pub fn close(&self) {
        self.show(false);
    }

    pub fn dismiss(&self, reason: Dismiss) {
        if self.wanted.get() && !self.on_dismiss.is_empty() {
            self.on_dismiss.call(reason);
        }
    }

    fn sync_attributes(&self) {
        let dialog = self.parts.dialog;
        set_attribute(
            dialog,
            "popover",
            (!self.modal()).then(|| "manual".to_owned()),
        );
        if !crate::frame::started() {
            set_attribute(dialog, "open", self.wanted.get().then(String::new));
        }
    }

    fn anchor_target(&self) -> Option<String> {
        let anchor = self.anchor.borrow().clone();
        let rect = match anchor {
            OverlayAnchor::Node(node) => {
                let node = node.try_get()?;
                let name = anchor_name(node);
                set_style(node, "anchor-name", Some(name.clone()));
                return Some(name);
            }
            OverlayAnchor::Point(point) => Rect::from_min_size(point, beui_types::Vec2::ZERO),
            OverlayAnchor::Rect(rect) | OverlayAnchor::Pinned { rect, .. } => rect,
        };
        let proxy = self.parts.proxy;
        set_style(proxy, "anchor-name", Some(self.name.clone()));
        set_style(proxy, "left", Some(px(rect.left())));
        set_style(proxy, "top", Some(px(rect.top())));
        set_style(proxy, "width", Some(px(rect.width())));
        set_style(proxy, "height", Some(px(rect.height())));
        Some(self.name.clone())
    }

    fn place(&self) {
        if !self.wanted.get() {
            return;
        }
        let name = self.anchor_target();
        let styles = anchored_styles(self.placement.get(), name.as_deref());
        for property in PLACED {
            let value = styles
                .iter()
                .find(|(name, _)| *name == property)
                .map(|(_, value)| value.clone());
            set_style(self.parts.dialog, property, value);
        }
        self.place_beak();
    }

    fn place_beak(&self) {
        let beak = self.parts.beak;
        let reach = self.reach.get();
        let shown = reach > 0.0 && matches!(self.placement.get(), Placement::Popover { .. });
        set_attribute(beak, "hidden", (!shown).then(String::new));
        if !shown {
            return;
        }
        let bubble = self.pointer.as_ref().and_then(NodeRef::try_get);
        let copied = |property: &str| bubble.and_then(|bubble| style_value(bubble, property));
        let size = reach * std::f32::consts::SQRT_2;
        set_style(beak, "--beui-beak", Some(px(size)));
        set_style(beak, "background-color", copied("background-color"));
        set_style(beak, "border", copied("outline"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn lay_out(&self) -> bool {
        let _ = (self.level.get(), self.traps.get(), &self.trigger);
        false
    }

    #[cfg(target_arch = "wasm32")]
    fn lay_out(&self) -> bool {
        web::lay_out(self)
    }
}

fn anchored_styles(placement: Placement, name: Option<&str>) -> Vec<(&'static str, String)> {
    let mut styles: Vec<(&'static str, String)> = Vec::new();
    let mut put = |property: &'static str, value: String| styles.push((property, value));
    match placement {
        Placement::Center => {
            put("top", "50%".to_owned());
            put("left", "50%".to_owned());
            put("translate", "-50% -50%".to_owned());
            put("max-width", "100vw".to_owned());
            put("max-height", "100vh".to_owned());
            return styles;
        }
        Placement::Fill => {
            for side in ["top", "left", "right", "bottom"] {
                put(side, "0".to_owned());
            }
            put("width", "100vw".to_owned());
            put("height", "100vh".to_owned());
            return styles;
        }
        _ => {}
    }
    let Some(name) = name else {
        put("top", "0".to_owned());
        put("left", "0".to_owned());
        return styles;
    };
    put("position-anchor", name.to_owned());
    match placement {
        Placement::At => {
            put("top", "anchor(top)".to_owned());
            put("left", "anchor(left)".to_owned());
        }
        Placement::Over(inset) => {
            put("top", format!("calc(anchor(top) - {inset}px)"));
            put("left", format!("calc(anchor(left) - {inset}px)"));
        }
        Placement::Around | Placement::CenterOn => {
            put("top", "anchor(center)".to_owned());
            put("left", "anchor(center)".to_owned());
            put("translate", "-50% -50%".to_owned());
        }
        Placement::Above => {
            put("position-area", "top".to_owned());
            put("position-try-fallbacks", "flip-block".to_owned());
        }
        Placement::BelowStart => {
            put("position-area", "bottom span-right".to_owned());
            put(
                "position-try-fallbacks",
                "flip-block, flip-inline, flip-block flip-inline".to_owned(),
            );
        }
        Placement::RightStart => {
            put("position-area", "right span-bottom".to_owned());
            put(
                "position-try-fallbacks",
                "flip-inline, flip-block, flip-block flip-inline".to_owned(),
            );
        }
        Placement::InsideTop => {
            put("top", "anchor(top)".to_owned());
            put("left", "anchor(center)".to_owned());
            put("translate", "-50% 0".to_owned());
        }
        Placement::InsideTopEnd => {
            put("top", "anchor(top)".to_owned());
            put("right", "anchor(right)".to_owned());
        }
        Placement::InsideBottom => {
            put("bottom", "anchor(bottom)".to_owned());
            put("left", "anchor(center)".to_owned());
            put("translate", "-50% 0".to_owned());
        }
        Placement::InsideBottomEnd => {
            put("bottom", "anchor(bottom)".to_owned());
            put("right", "anchor(right)".to_owned());
        }
        Placement::HangFrom(gap) => {
            put("top", format!("calc(anchor(top) + {gap}px)"));
            put("left", "anchor(center)".to_owned());
            put("translate", "-50% 0".to_owned());
        }
        Placement::Popover { gap, margin } => {
            put("position-area", "bottom".to_owned());
            put("position-try-fallbacks", "flip-block".to_owned());
            put("margin-top", format!("{gap}px"));
            put(
                "max-width",
                format!("calc(100vw - {}px)", 2 * u32::from(margin)),
            );
        }
        Placement::Center | Placement::Fill => {}
    }
    styles
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use beui_types::{Dismiss, LayerPolicy, Placement, Rect, Vec2, pos2};
    use wasm_bindgen::{JsCast, JsValue};

    use super::{Layer, OverlayAnchor, PLACED, open_layers};
    use crate::css::px;
    use crate::document::set_style;
    use crate::dom::Listener;
    use crate::node::NodeRef;

    #[derive(Default)]
    pub(super) struct Held {
        pub(super) shown: Option<bool>,
        dialog: Option<web_sys::Element>,
        returns_to: Option<web_sys::HtmlElement>,
        listeners: Vec<Listener>,
    }

    thread_local! {
        static GLOBAL: RefCell<Vec<Listener>> = RefCell::default();
        static ANCHORS: Cell<Option<bool>> = const { Cell::new(None) };
        static QUERIES: Cell<Option<bool>> = const { Cell::new(None) };
    }

    fn supported(
        held: &'static std::thread::LocalKey<Cell<Option<bool>>>,
        property: &str,
        value: &str,
    ) -> bool {
        held.with(|held| match held.get() {
            Some(known) => known,
            None => {
                let known = crate::dom::css_supports(property, value);
                held.set(Some(known));
                known
            }
        })
    }

    fn anchors_supported() -> bool {
        supported(&ANCHORS, "anchor-name", "--beui")
    }

    fn queries_supported() -> bool {
        supported(&QUERIES, "container-type", "anchored")
    }

    fn within(element: &web_sys::Element, target: Option<&web_sys::Node>) -> bool {
        target.is_some_and(|target| element.contains(Some(target)))
    }

    fn pressed(event: web_sys::Event) {
        let Some(event) = event.dyn_ref::<web_sys::PointerEvent>() else {
            return;
        };
        let target = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Node>().ok());
        let point = pos2(event.client_x() as f32, event.client_y() as f32);
        for layer in open_layers().into_iter().rev() {
            let Some(dialog) = crate::dom::element(layer.parts.dialog) else {
                continue;
            };
            let dialog_node: &web_sys::Node = dialog.as_ref();
            let inside = match target.as_ref() == Some(dialog_node) {
                true => crate::dom::client_rect(&dialog).contains(point),
                false => within(&dialog, target.as_ref()),
            };
            if inside {
                break;
            }
            let on_trigger = layer
                .trigger
                .as_ref()
                .and_then(NodeRef::try_get)
                .and_then(crate::dom::element)
                .is_some_and(|trigger| within(&trigger, target.as_ref()));
            if !on_trigger {
                layer.dismiss(Dismiss::OutsidePress);
            }
            if layer.modal() {
                break;
            }
        }
        reactive::flush();
    }

    fn keyed(event: web_sys::Event) {
        let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if key.key() != "Escape" || event.default_prevented() {
            return;
        }
        let top = open_layers()
            .into_iter()
            .rev()
            .find(|layer| layer.policy.get() != LayerPolicy::Inert);
        let Some(layer) = top.filter(|layer| !layer.modal() && !layer.on_dismiss.is_empty()) else {
            return;
        };
        event.prevent_default();
        layer.dismiss(Dismiss::Escape);
        reactive::flush();
    }

    fn focused(event: web_sys::Event) {
        let target = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Node>().ok());
        let top = open_layers()
            .into_iter()
            .rev()
            .find(|layer| layer.policy.get() != LayerPolicy::Inert);
        let Some(layer) = top.filter(|layer| layer.traps.get() && !layer.modal()) else {
            return;
        };
        let Some(dialog) = crate::dom::element(layer.parts.dialog) else {
            return;
        };
        if within(&dialog, target.as_ref()) {
            return;
        }
        let wanted = crate::focus::wanted()
            .and_then(crate::dom::dom)
            .is_some_and(|wanted| dialog.contains(Some(&wanted)));
        if wanted {
            crate::focus::apply_wanted();
            return;
        }
        let first = dialog
            .query_selector("[tabindex], a[href], button, input, select, textarea")
            .ok()
            .flatten()
            .and_then(|first| first.dyn_into::<web_sys::HtmlElement>().ok());
        if let Some(first) = first {
            let _ = first.focus();
        }
    }

    fn listen_globally() {
        GLOBAL.with(|global| {
            let mut global = global.borrow_mut();
            if !global.is_empty() {
                return;
            }
            let (Some(window), Some(document)) = (crate::dom::window(), crate::dom::document())
            else {
                return;
            };
            global.push(crate::dom::listen(
                document.as_ref(),
                "pointerdown",
                true,
                pressed,
            ));
            global.push(crate::dom::listen(window.as_ref(), "keydown", false, keyed));
            global.push(crate::dom::listen(
                document.as_ref(),
                "focusin",
                true,
                focused,
            ));
        });
    }

    fn attach(layer: &Layer, held: &mut Held, dialog: &web_sys::Element) {
        if held.dialog.as_ref() == Some(dialog) {
            return;
        }
        let host = layer.parts.host;
        let cancel = crate::dom::listen(
            dialog.as_ref(),
            "cancel",
            false,
            move |event: web_sys::Event| {
                event.prevent_default();
                if let Some(layer) = super::find(host) {
                    layer.dismiss(Dismiss::Escape);
                }
                reactive::flush();
            },
        );
        let close =
            crate::dom::listen(dialog.as_ref(), "close", false, move |_: web_sys::Event| {
                if let Some(layer) = super::find(host) {
                    layer.web.borrow_mut().shown = None;
                }
                crate::frame::request_frame();
            });
        held.listeners = vec![cancel, close];
        held.dialog = Some(dialog.clone());
        held.shown = None;
    }

    fn state(dialog: &web_sys::Element) -> Option<bool> {
        let matches = |selector: &str| dialog.matches(selector).unwrap_or(false);
        if matches(":modal") {
            Some(true)
        } else if matches(":popover-open") {
            Some(false)
        } else {
            None
        }
    }

    fn hide(dialog: &web_sys::Element) {
        let value = JsValue::from(dialog);
        match state(dialog) {
            Some(true) => {
                crate::dom::call(&value, "close", &[]);
            }
            Some(false) => {
                crate::dom::call(&value, "hidePopover", &[]);
            }
            None => {}
        }
        let _ = dialog.remove_attribute("open");
    }

    fn show(dialog: &web_sys::Element, modal: bool) {
        hide(dialog);
        let value = JsValue::from(dialog);
        let _ = dialog.set_attribute("inert", "");
        match modal {
            true => {
                let _ = dialog.remove_attribute("popover");
                crate::dom::call(&value, "showModal", &[]);
            }
            false => {
                let _ = dialog.set_attribute("popover", "manual");
                crate::dom::call(&value, "showPopover", &[]);
            }
        }
        let _ = dialog.remove_attribute("inert");
        crate::focus::apply_wanted();
    }

    fn anchor_rect(layer: &Layer) -> Option<Rect> {
        let origin = crate::dom::origin();
        let shifted = |rect: Rect| rect.translate(Vec2::new(origin.x, origin.y));
        let anchor = layer.anchor.borrow().clone();
        match anchor {
            OverlayAnchor::Node(node) => node
                .try_get()
                .and_then(crate::dom::element)
                .map(|element| crate::dom::client_rect(&element)),
            OverlayAnchor::Point(point) => Some(shifted(Rect::from_min_size(point, Vec2::ZERO))),
            OverlayAnchor::Rect(rect) | OverlayAnchor::Pinned { rect, .. } => Some(shifted(rect)),
        }
    }

    fn measured(layer: &Layer, dialog: &web_sys::Element) {
        let placement = layer.placement.get();
        if matches!(placement, Placement::Center | Placement::Fill) {
            return;
        }
        let Some(anchor) = anchor_rect(layer) else {
            return;
        };
        for property in PLACED {
            if !matches!(property, "width" | "height" | "max-width" | "max-height") {
                set_style(layer.parts.dialog, property, None);
            }
        }
        let size = crate::dom::client_rect(dialog).size();
        let rect = beui_types::layer::resolve_rect(crate::dom::viewport(), anchor, placement, size);
        set_style(layer.parts.dialog, "left", Some(px(rect.left())));
        set_style(layer.parts.dialog, "top", Some(px(rect.top())));
    }

    fn point_beak(layer: &Layer, dialog: &web_sys::Element, anchors: bool) {
        let Some(beak) = crate::dom::element(layer.parts.beak) else {
            return;
        };
        if beak.has_attribute("hidden") {
            return;
        }
        let Some(anchor) = anchor_rect(layer) else {
            return;
        };
        let rect = crate::dom::client_rect(dialog);
        let size = layer.reach.get() * std::f32::consts::SQRT_2;
        set_style(
            layer.parts.beak,
            "left",
            Some(px(anchor.center().x - rect.left() - size / 2.0)),
        );
        let queries = anchors && queries_supported();
        set_style(
            layer.parts.dialog,
            "container-type",
            queries.then(|| "anchored".to_owned()),
        );
        if queries {
            set_style(layer.parts.beak, "top", None);
            set_style(layer.parts.beak, "bottom", None);
            return;
        }
        let (top, bottom) = match (rect.top() >= anchor.bottom(), rect.bottom() <= anchor.top()) {
            (true, _) => (Some(px(-size / 2.0)), None),
            (false, true) => (None, Some(px(-size / 2.0))),
            (false, false) => (None, None),
        };
        set_style(layer.parts.beak, "top", top);
        set_style(layer.parts.beak, "bottom", bottom);
    }

    fn raise_above(layer: &Layer) {
        let level = layer.level.get();
        let mut after = false;
        for other in open_layers() {
            if std::ptr::eq(Rc::as_ptr(&other), layer) {
                after = true;
                continue;
            }
            let other_level = other.level.get();
            if other_level < level || (other_level == level && !after) {
                continue;
            }
            other.web.borrow_mut().shown = None;
            crate::frame::request_frame();
        }
    }

    fn anchored_above(layer: &Layer) -> bool {
        let OverlayAnchor::Node(node) = layer.anchor.borrow().clone() else {
            return false;
        };
        let Some(anchor) = node.try_get().and_then(crate::dom::dom) else {
            return false;
        };
        let open = open_layers();
        let Some(index) = open
            .iter()
            .position(|other| std::ptr::eq(Rc::as_ptr(other), layer))
        else {
            return false;
        };
        open[index + 1..].iter().any(|other| {
            crate::dom::dom(other.parts.dialog).is_some_and(|dialog| dialog.contains(Some(&anchor)))
        })
    }

    pub(super) fn lay_out(layer: &Layer) -> bool {
        let Some(dialog) = crate::dom::element(layer.parts.dialog) else {
            return false;
        };
        if !dialog.is_connected() {
            return false;
        }
        if layer.wanted.get() && anchored_above(layer) {
            layer.raise();
        }
        let mut held = layer.web.borrow_mut();
        attach(layer, &mut held, &dialog);
        let wanted = layer.wanted.get().then(|| layer.modal());
        let mut changed = false;
        if held.shown != wanted || state(&dialog) != wanted {
            let active = crate::dom::document().and_then(|document| document.active_element());
            let inside = active
                .as_ref()
                .is_some_and(|active| within(&dialog, Some(active.as_ref())));
            match wanted {
                Some(modal) => {
                    if held.shown.is_none() && !inside {
                        held.returns_to = active.and_then(|active| active.dyn_into().ok());
                    }
                    listen_globally();
                    show(&dialog, modal);
                }
                None => {
                    hide(&dialog);
                    let returns_to = held.returns_to.take();
                    let lost = crate::dom::document()
                        .and_then(|document| document.active_element())
                        .is_none_or(|active| active.tag_name() == "BODY");
                    if (inside || lost)
                        && let Some(element) = returns_to.filter(|element| element.is_connected())
                    {
                        let _ = element.focus();
                    }
                }
            }
            held.shown = wanted;
            changed = true;
        }
        drop(held);
        if changed && wanted.is_some() {
            raise_above(layer);
        }
        if wanted.is_none() {
            return changed;
        }
        let anchors = anchors_supported();
        if !anchors {
            measured(layer, &dialog);
        }
        point_beak(layer, &dialog, anchors);
        changed
    }
}
