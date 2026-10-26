use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::context::{RendererInfo, WindowOutput};
use beui_core::display::{Display, Group, Layer, Part};
use beui_core::geometry::Vec2;
use beui_core::image::{Image, ImageId};
use beui_core::node::NodeId;
use beui_core::painter::Shape;
use beui_core::renderer::Renderer;

use crate::dom::Dom;
use crate::style::{self, Look};

const STYLE: &str = "\
.beui-root{position:relative;overflow:hidden;user-select:none;-webkit-user-select:none;touch-action:none;outline:none}\
.beui-stage{position:absolute;left:0;top:0;width:0;height:0;transform-origin:0 0;pointer-events:none}\
.beui-stage div{position:absolute;left:0;top:0;box-sizing:border-box;white-space:pre;pointer-events:none;font-variant-ligatures:none}\
.beui-stage .beui-layer{isolation:isolate}\
.beui-stage .beui-scaled{transform-origin:0 0}";
const IMAGE_LIMIT: usize = 512;

type Key = (u64, NodeId);

struct Child<E> {
    id: u64,
    element: E,
}

impl<E: Clone> Clone for Child<E> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            element: self.element.clone(),
        }
    }
}

struct Grouped<E> {
    group: Group,
    factor: f32,
    outer: Child<E>,
    inner: E,
    children: Vec<Child<E>>,
}

struct Node<E> {
    frame: Child<E>,
    content: E,
    display: Option<Rc<Display>>,
    factor: f32,
    parent: Option<Key>,
    placed: Option<(Vec2, f32)>,
    painted: Vec<Option<Child<E>>>,
    groups: Vec<Option<Grouped<E>>>,
    children: Vec<Child<E>>,
    kids: Vec<Key>,
}

struct Root<E> {
    wrap: Child<E>,
    holds: Option<u64>,
}

pub struct DomRenderer<D: Dom> {
    dom: D,
    root: D::Element,
    stage: D::Element,
    next: u64,
    layers: Vec<Child<D::Element>>,
    roots: HashMap<Key, Root<D::Element>>,
    groups: Vec<Option<Grouped<D::Element>>>,
    loose: Vec<Option<Child<D::Element>>>,
    nodes: HashMap<Key, Node<D::Element>>,
    images: HashMap<ImageId, Rc<str>>,
    prepared: Option<(f32, f32, Color32)>,
    size: Option<(u32, u32)>,
}

struct Shown {
    loose: usize,
    groups: usize,
    roots: HashSet<Key>,
}

impl<D: Dom> DomRenderer<D> {
    pub fn new(mut dom: D) -> Option<Self> {
        dom.add_stylesheet(STYLE);
        let root = dom.root();
        let stage = dom.div()?;
        dom.set_class(&stage, "beui-stage");
        dom.insert_before(&root, &stage, None);
        Some(Self {
            dom,
            root,
            stage,
            next: 0,
            layers: Vec::new(),
            roots: HashMap::new(),
            groups: Vec::new(),
            loose: Vec::new(),
            nodes: HashMap::new(),
            images: HashMap::new(),
            prepared: None,
            size: None,
        })
    }

    pub fn dom(&self) -> &D {
        &self.dom
    }

    fn show(&mut self, output: &WindowOutput, scale: f32, background: Color32) {
        let ratio = self.dom.device_pixel_ratio();
        let zoom = scale / ratio;
        let prepared = Some((scale, zoom, background));
        if !output.changed && self.prepared == prepared {
            return;
        }
        let held = self.prepared.replace((scale, zoom, background));
        if held.is_none_or(|(_, _, old)| old != background) {
            self.dom
                .set_style(&self.root, "background", &style::rgba(background));
        }
        if held.is_none_or(|(_, old, _)| old != zoom) {
            self.dom.set_style(
                &self.stage,
                "transform",
                &match zoom == 1.0 {
                    true => String::new(),
                    false => format!("scale({zoom})"),
                },
            );
        }
        if held.is_some_and(|(old, _, _)| old != scale) {
            self.clear();
        }
        let mut shown = Shown {
            loose: 0,
            groups: 0,
            roots: HashSet::new(),
        };
        let mut groups = std::mem::take(&mut self.groups);
        let desired = self.frame_layers(&output.layers, scale, &mut groups, &mut shown);
        groups.truncate(shown.groups);
        self.groups = groups;
        self.loose.truncate(shown.loose);
        reconcile(&mut self.dom, &self.stage, &mut self.layers, desired);
        let gone: Vec<Key> = self
            .roots
            .keys()
            .filter(|key| !shown.roots.contains(key))
            .copied()
            .collect();
        for key in gone {
            self.roots.remove(&key);
            self.release(key, None);
        }
    }

    fn frame_layers(
        &mut self,
        layers: &[Layer],
        factor: f32,
        groups: &mut Vec<Option<Grouped<D::Element>>>,
        shown: &mut Shown,
    ) -> Vec<Child<D::Element>> {
        let mut desired = Vec::new();
        for layer in layers {
            match layer {
                Layer::Shape(shape) => {
                    if let Some(child) = self.loose(shown.loose, shape, factor) {
                        desired.push(child);
                    }
                    shown.loose += 1;
                }
                Layer::Display(at, display) => {
                    if let Some(wrap) = self.layer(display, *at, factor) {
                        shown.roots.insert(key(display));
                        desired.push(wrap);
                    }
                }
                Layer::Group(group, inner) => {
                    let index = shown.groups;
                    shown.groups += 1;
                    let Some(mut grouped) = self.grouped(groups, index, *group, factor) else {
                        continue;
                    };
                    let within = inner_factor(*group, factor);
                    let wanted = self.frame_layers(inner, within, groups, shown);
                    reconcile(&mut self.dom, &grouped.inner, &mut grouped.children, wanted);
                    desired.push(grouped.outer.clone());
                    put(groups, index, grouped);
                }
            }
        }
        desired
    }

    fn grouped(
        &mut self,
        groups: &mut [Option<Grouped<D::Element>>],
        index: usize,
        group: Group,
        factor: f32,
    ) -> Option<Grouped<D::Element>> {
        let held = groups.get_mut(index).and_then(Option::take);
        let mut grouped = match held {
            Some(grouped) => grouped,
            None => {
                let outer = self.child()?;
                self.dom.set_class(&outer.element, "beui-group");
                let inner = self.dom.div()?;
                self.dom.insert_before(&outer.element, &inner, None);
                Grouped {
                    group,
                    factor: 0.0,
                    outer,
                    inner,
                    children: Vec::new(),
                }
            }
        };
        if grouped.factor != factor || grouped.group != group {
            let (outer, inner) = style::group(group, factor);
            self.dom.set_css(&grouped.outer.element, &outer);
            self.dom.set_css(&grouped.inner, &inner);
            let scaled = inner_factor(group, 1.0) != 1.0;
            self.dom.set_class(
                &grouped.inner,
                match scaled {
                    true => "beui-scaled",
                    false => "",
                },
            );
            grouped.group = group;
            grouped.factor = factor;
        }
        Some(grouped)
    }

    fn clear(&mut self) {
        self.dom.set_text(&self.stage, None);
        self.layers.clear();
        self.roots.clear();
        self.groups.clear();
        self.loose.clear();
        self.nodes.clear();
    }

    fn child(&mut self) -> Option<Child<D::Element>> {
        let element = self.dom.div()?;
        self.next += 1;
        Some(Child {
            id: self.next,
            element,
        })
    }

    fn loose(&mut self, index: usize, shape: &Shape, factor: f32) -> Option<Child<D::Element>> {
        let reused = self.loose.get_mut(index).and_then(Option::take);
        let child = self.paint(reused, shape, factor);
        match index < self.loose.len() {
            true => self.loose[index] = child.clone(),
            false => self.loose.push(child.clone()),
        }
        child
    }

    fn layer(&mut self, display: &Rc<Display>, at: Vec2, factor: f32) -> Option<Child<D::Element>> {
        let frame = self.visit(display, factor, None)?;
        let key = key(display);
        self.place(key, at, factor);
        if !self.roots.contains_key(&key) {
            let wrap = self.child()?;
            self.dom.set_class(&wrap.element, "beui-layer");
            self.roots.insert(key, Root { wrap, holds: None });
        }
        let root = self.roots.get_mut(&key)?;
        if root.holds != Some(frame.id) {
            self.dom.set_text(&root.wrap.element, None);
            self.dom
                .insert_before(&root.wrap.element, &frame.element, None);
            root.holds = Some(frame.id);
        }
        Some(root.wrap.clone())
    }

    fn visit(
        &mut self,
        display: &Rc<Display>,
        factor: f32,
        parent: Option<Key>,
    ) -> Option<Child<D::Element>> {
        let key = key(display);
        let mut node = match self.nodes.remove(&key) {
            Some(node) => node,
            None => {
                let frame = self.child()?;
                self.dom.set_class(&frame.element, "beui-frame");
                let content = self.dom.div()?;
                self.dom.insert_before(&frame.element, &content, None);
                Node {
                    frame,
                    content,
                    display: None,
                    factor,
                    parent,
                    placed: None,
                    painted: Vec::new(),
                    groups: Vec::new(),
                    children: Vec::new(),
                    kids: Vec::new(),
                }
            }
        };
        node.parent = parent;
        let unchanged = node.factor == factor
            && node
                .display
                .as_ref()
                .is_some_and(|held| Rc::ptr_eq(held, display));
        if !unchanged {
            self.update(&mut node, display, factor);
        }
        let frame = node.frame.clone();
        self.nodes.insert(key, node);
        Some(frame)
    }

    fn update(&mut self, node: &mut Node<D::Element>, display: &Rc<Display>, factor: f32) {
        let kept = node
            .display
            .as_ref()
            .is_some_and(|held| node.factor == factor && Rc::ptr_eq(&held.shapes, &display.shapes));
        let mut old = std::mem::take(&mut node.painted);
        let mut painted = Vec::with_capacity(display.shapes.len());
        for (index, shape) in display.shapes.iter().enumerate() {
            let previous = old.get_mut(index).and_then(Option::take);
            match previous {
                Some(previous) if kept => painted.push(Some(previous)),
                previous => painted.push(self.paint(previous, shape, factor)),
            }
        }
        let mut kids = Vec::new();
        let mut groups = std::mem::take(&mut node.groups);
        let mut index = 0;
        let desired = self.fill(
            &display.parts,
            key(display),
            factor,
            &painted,
            &mut groups,
            &mut index,
            &mut kids,
        );
        groups.truncate(index);
        node.groups = groups;
        reconcile(&mut self.dom, &node.content, &mut node.children, desired);
        let shown: HashSet<Key> = kids.iter().copied().collect();
        for kid in std::mem::replace(&mut node.kids, kids) {
            if !shown.contains(&kid) {
                self.release(kid, Some(key(display)));
            }
        }
        node.painted = painted;
        node.display = Some(Rc::clone(display));
        node.factor = factor;
    }

    #[allow(clippy::too_many_arguments)]
    fn fill(
        &mut self,
        parts: &[Part],
        parent: Key,
        factor: f32,
        painted: &[Option<Child<D::Element>>],
        groups: &mut Vec<Option<Grouped<D::Element>>>,
        index: &mut usize,
        kids: &mut Vec<Key>,
    ) -> Vec<Child<D::Element>> {
        let mut desired = Vec::new();
        for part in parts {
            match part {
                Part::Shapes { start, end, .. } => desired.extend(
                    painted[*start as usize..*end as usize]
                        .iter()
                        .flatten()
                        .cloned(),
                ),
                Part::Child(_, at, child) => {
                    if let Some(frame) = self.visit(child, factor, Some(parent)) {
                        self.place(key(child), *at, factor);
                        kids.push(key(child));
                        desired.push(frame);
                    }
                }
                Part::Group(group, inner) => {
                    let at = *index;
                    *index += 1;
                    let Some(mut grouped) = self.grouped(groups, at, *group, factor) else {
                        continue;
                    };
                    let within = inner_factor(*group, factor);
                    let wanted = self.fill(inner, parent, within, painted, groups, index, kids);
                    reconcile(&mut self.dom, &grouped.inner, &mut grouped.children, wanted);
                    desired.push(grouped.outer.clone());
                    put(groups, at, grouped);
                }
            }
        }
        desired
    }

    fn place(&mut self, key: Key, at: Vec2, factor: f32) {
        let Some(node) = self.nodes.get_mut(&key) else {
            return;
        };
        if node.placed == Some((at, factor)) {
            return;
        }
        node.placed = Some((at, factor));
        self.dom
            .set_css(&node.frame.element, &style::placement(at, factor));
    }

    fn release(&mut self, key: Key, parent: Option<Key>) {
        let mut pending = vec![(key, parent)];
        while let Some((key, parent)) = pending.pop() {
            if self
                .nodes
                .get(&key)
                .is_none_or(|node| node.parent != parent)
            {
                continue;
            }
            if let Some(node) = self.nodes.remove(&key) {
                pending.extend(node.kids.into_iter().map(|kid| (kid, Some(key))));
            }
        }
    }

    fn paint(
        &mut self,
        reused: Option<Child<D::Element>>,
        shape: &Shape,
        factor: f32,
    ) -> Option<Child<D::Element>> {
        let image = match shape {
            Shape::Image { image, .. } => self.image(image),
            _ => None,
        };
        let look = style::look(shape, factor, image.as_deref())?;
        let child = match reused {
            Some(child) => child,
            None => self.child()?,
        };
        self.dom.set_text(&child.element, None);
        match look {
            Look::Box(css) => self.dom.set_css(&child.element, &css),
            Look::Text { css, lines } => {
                self.dom.set_css(&child.element, &css);
                for line in lines {
                    let Some(row) = self.dom.div() else {
                        continue;
                    };
                    self.dom.set_css(&row, &line.css);
                    self.dom.set_text(&row, Some(&line.text));
                    self.dom.insert_before(&child.element, &row, None);
                }
            }
        }
        Some(child)
    }

    fn image(&mut self, image: &Image) -> Option<Rc<str>> {
        if let Some(url) = self.images.get(&image.id()) {
            return Some(Rc::clone(url));
        }
        if self.images.len() >= IMAGE_LIMIT {
            self.images.clear();
        }
        let url: Rc<str> = self.dom.image_url(image)?.into();
        self.images.insert(image.id(), Rc::clone(&url));
        Some(url)
    }
}

fn key(display: &Display) -> Key {
    (display.document, display.node)
}

fn inner_factor(group: Group, factor: f32) -> f32 {
    match group {
        Group::Transform { scale, .. } | Group::Scroll { scale, .. } => factor * scale,
        _ => factor,
    }
}

fn put<E>(groups: &mut Vec<Option<Grouped<E>>>, index: usize, grouped: Grouped<E>) {
    if groups.len() <= index {
        groups.resize_with(index + 1, || None);
    }
    groups[index] = Some(grouped);
}

impl<D: Dom> Renderer for DomRenderer<D> {
    fn name(&self) -> &'static str {
        "DOM"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo {
            rows: vec![("Renderer", "DOM".to_owned())],
        }
    }

    fn set_active(&mut self, active: bool) {
        let display = match active {
            true => "",
            false => "none",
        };
        self.dom.set_style(&self.stage, "display", display);
        if !active {
            self.prepared = None;
            self.clear();
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = Some((width, height));
    }

    fn physical(&self) -> Option<Vec2> {
        self.size
            .map(|(width, height)| Vec2::new(width as f32, height as f32))
    }

    fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool {
        self.show(output, scale, background);
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

fn reconcile<D: Dom>(
    dom: &mut D,
    parent: &D::Element,
    current: &mut Vec<Child<D::Element>>,
    desired: Vec<Child<D::Element>>,
) {
    if current.len() == desired.len()
        && current
            .iter()
            .zip(&desired)
            .all(|(held, wanted)| held.id == wanted.id)
    {
        return;
    }
    let wanted: HashSet<u64> = desired.iter().map(|child| child.id).collect();
    let mut positions = HashMap::new();
    for held in current.iter() {
        match wanted.contains(&held.id) {
            true => {
                positions.insert(held.id, positions.len());
            }
            false => dom.remove_from(parent, &held.element),
        }
    }
    let assigned: Vec<Option<usize>> = desired
        .iter()
        .map(|child| positions.get(&child.id).copied())
        .collect();
    let kept = crate::order::longest_increasing(&assigned);
    let mut next: Option<&D::Element> = None;
    for (index, child) in desired.iter().enumerate().rev() {
        if !kept[index] {
            dom.insert_before(parent, &child.element, next);
        }
        next = Some(&child.element);
    }
    *current = desired;
}
