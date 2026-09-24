use crate::base::Direction;
use crate::geometry::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TabId(u64);

impl TabId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeafId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SplitId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    pub fn direction(self) -> Direction {
        match self {
            Side::Left | Side::Right => Direction::Horizontal,
            Side::Above | Side::Below => Direction::Vertical,
        }
    }

    fn leads(self) -> bool {
        matches!(self, Side::Left | Side::Above)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockDrop {
    Tab { leaf: LeafId, index: usize },
    Pane { leaf: LeafId },
    Split { leaf: LeafId, side: Side },
    Window { pos: Pos2 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabPosition {
    pub surface: SurfaceId,
    pub leaf: LeafId,
    pub index: usize,
}

pub const MIN_FRACTION: f32 = 0.05;
pub const MIN_WINDOW_SIZE: Vec2 = Vec2::new(200.0, 140.0);
pub const FLOATING_SIZE: Vec2 = Vec2::new(420.0, 300.0);

#[derive(Clone, Debug, PartialEq)]
struct Leaf {
    id: LeafId,
    tabs: Vec<TabId>,
    active: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct Split {
    id: SplitId,
    direction: Direction,
    fraction: f32,
    first: Box<Node>,
    second: Box<Node>,
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Leaf(Leaf),
    Split(Split),
}

impl Node {
    fn walk<'a>(&'a self, out: &mut Vec<&'a Leaf>) {
        match self {
            Node::Leaf(leaf) => out.push(leaf),
            Node::Split(split) => {
                split.first.walk(out);
                split.second.walk(out);
            }
        }
    }

    fn leaf(&self, id: LeafId) -> Option<&Leaf> {
        match self {
            Node::Leaf(leaf) => (leaf.id == id).then_some(leaf),
            Node::Split(split) => split.first.leaf(id).or_else(|| split.second.leaf(id)),
        }
    }

    fn leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf> {
        match self {
            Node::Leaf(leaf) => (leaf.id == id).then_some(leaf),
            Node::Split(split) => match split.first.leaf_mut(id) {
                Some(leaf) => Some(leaf),
                None => split.second.leaf_mut(id),
            },
        }
    }

    fn split(&self, id: SplitId) -> Option<&Split> {
        let Node::Split(split) = self else {
            return None;
        };
        if split.id == id {
            return Some(split);
        }
        split.first.split(id).or_else(|| split.second.split(id))
    }

    fn split_mut(&mut self, id: SplitId) -> Option<&mut Split> {
        let Node::Split(split) = self else {
            return None;
        };
        if split.id == id {
            return Some(split);
        }
        match split.first.split_mut(id) {
            Some(found) => Some(found),
            None => split.second.split_mut(id),
        }
    }

    fn replace_leaf<F: Fn(Node) -> Node + Clone>(&mut self, target: LeafId, build: F) -> bool {
        match self {
            Node::Leaf(leaf) => {
                if leaf.id != target {
                    return false;
                }
                let placeholder = Node::Leaf(Leaf {
                    id: leaf.id,
                    tabs: Vec::new(),
                    active: 0,
                });
                let existing = std::mem::replace(self, placeholder);
                *self = build(existing);
                true
            }
            Node::Split(split) => {
                split.first.replace_leaf(target, build.clone())
                    || split.second.replace_leaf(target, build)
            }
        }
    }

    fn without_leaf(self, id: LeafId) -> Option<Node> {
        match self {
            Node::Leaf(leaf) => (leaf.id != id).then_some(Node::Leaf(leaf)),
            Node::Split(split) => {
                let Split {
                    id: split_id,
                    direction,
                    fraction,
                    first,
                    second,
                } = split;
                match (first.without_leaf(id), second.without_leaf(id)) {
                    (Some(first), Some(second)) => Some(Node::Split(Split {
                        id: split_id,
                        direction,
                        fraction,
                        first: Box::new(first),
                        second: Box::new(second),
                    })),
                    (Some(node), None) | (None, Some(node)) => Some(node),
                    (None, None) => None,
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Surface {
    id: SurfaceId,
    root: Node,
    window: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockState {
    surfaces: Vec<Surface>,
    focus: Option<LeafId>,
    next: u64,
}

impl Default for DockState {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl DockState {
    pub fn new(tabs: impl IntoIterator<Item = TabId>) -> Self {
        let mut state = Self {
            surfaces: Vec::new(),
            focus: None,
            next: 0,
        };
        let leaf = state.new_leaf(tabs.into_iter().collect());
        let focus = leaf.id;
        let id = SurfaceId(state.mint());
        state.surfaces.push(Surface {
            id,
            root: Node::Leaf(leaf),
            window: None,
        });
        state.focus = Some(focus);
        state
    }

    fn mint(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn new_leaf(&mut self, tabs: Vec<TabId>) -> Leaf {
        Leaf {
            id: LeafId(self.mint()),
            tabs,
            active: 0,
        }
    }

    fn surface(&self, id: SurfaceId) -> Option<&Surface> {
        self.surfaces.iter().find(|surface| surface.id == id)
    }

    fn surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|surface| surface.id == id)
    }

    fn leaf(&self, id: LeafId) -> Option<&Leaf> {
        self.surfaces
            .iter()
            .find_map(|surface| surface.root.leaf(id))
    }

    fn leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf> {
        self.surfaces
            .iter_mut()
            .find_map(|surface| surface.root.leaf_mut(id))
    }

    pub fn main(&self) -> SurfaceId {
        self.surfaces[0].id
    }

    pub fn surfaces(&self) -> Vec<SurfaceId> {
        self.surfaces.iter().map(|surface| surface.id).collect()
    }

    pub fn windows(&self) -> Vec<SurfaceId> {
        self.surfaces
            .iter()
            .filter(|surface| surface.window.is_some())
            .map(|surface| surface.id)
            .collect()
    }

    pub fn window_rect(&self, surface: SurfaceId) -> Option<Rect> {
        self.surface(surface).and_then(|surface| surface.window)
    }

    pub fn set_window_rect(&mut self, surface: SurfaceId, rect: Rect) {
        if let Some(surface) = self.surface_mut(surface)
            && surface.window.is_some()
        {
            surface.window = Some(rect);
        }
    }

    pub fn open_window(&mut self, rect: Rect, tabs: Vec<TabId>) -> SurfaceId {
        let leaf = self.new_leaf(tabs);
        let focus = leaf.id;
        let id = SurfaceId(self.mint());
        self.surfaces.push(Surface {
            id,
            root: Node::Leaf(leaf),
            window: Some(rect),
        });
        self.focus = Some(focus);
        id
    }

    pub fn close_window(&mut self, surface: SurfaceId) {
        if self
            .surface(surface)
            .is_none_or(|surface| surface.window.is_none())
        {
            return;
        }
        self.surfaces.retain(|candidate| candidate.id != surface);
        self.settle_focus();
    }

    pub fn raise(&mut self, surface: SurfaceId) {
        let Some(index) = self
            .surfaces
            .iter()
            .position(|candidate| candidate.id == surface)
        else {
            return;
        };
        if index == 0 || index + 1 == self.surfaces.len() {
            return;
        }
        let raised = self.surfaces.remove(index);
        self.surfaces.push(raised);
    }

    pub fn surface_of(&self, leaf: LeafId) -> Option<SurfaceId> {
        self.surfaces
            .iter()
            .find(|surface| surface.root.leaf(leaf).is_some())
            .map(|surface| surface.id)
    }

    pub fn leaves(&self, surface: SurfaceId) -> Vec<LeafId> {
        let Some(surface) = self.surface(surface) else {
            return Vec::new();
        };
        let mut leaves = Vec::new();
        surface.root.walk(&mut leaves);
        leaves.into_iter().map(|leaf| leaf.id).collect()
    }

    pub fn tabs(&self, leaf: LeafId) -> Vec<TabId> {
        self.leaf(leaf)
            .map(|leaf| leaf.tabs.clone())
            .unwrap_or_default()
    }

    pub fn active_index(&self, leaf: LeafId) -> usize {
        self.leaf(leaf).map_or(0, |leaf| leaf.active)
    }

    pub fn active_tab(&self, leaf: LeafId) -> Option<TabId> {
        let leaf = self.leaf(leaf)?;
        leaf.tabs.get(leaf.active).copied()
    }

    pub fn set_active_index(&mut self, leaf: LeafId, index: usize) {
        if let Some(leaf) = self.leaf_mut(leaf)
            && index < leaf.tabs.len()
        {
            leaf.active = index;
        }
    }

    pub fn find(&self, tab: TabId) -> Option<TabPosition> {
        self.surfaces.iter().find_map(|surface| {
            let mut leaves = Vec::new();
            surface.root.walk(&mut leaves);
            leaves.into_iter().find_map(|leaf| {
                let index = leaf.tabs.iter().position(|candidate| *candidate == tab)?;
                Some(TabPosition {
                    surface: surface.id,
                    leaf: leaf.id,
                    index,
                })
            })
        })
    }

    pub fn contains(&self, tab: TabId) -> bool {
        self.find(tab).is_some()
    }

    pub fn all_tabs(&self) -> Vec<TabId> {
        let mut leaves = Vec::new();
        for surface in &self.surfaces {
            surface.root.walk(&mut leaves);
        }
        leaves
            .into_iter()
            .flat_map(|leaf| leaf.tabs.iter().copied())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.all_tabs().is_empty()
    }

    pub fn focused_leaf(&self) -> Option<LeafId> {
        self.focus
    }

    pub fn focused_tab(&self) -> Option<TabId> {
        self.focus.and_then(|leaf| self.active_tab(leaf))
    }

    pub fn focus(&mut self, leaf: LeafId) {
        if self.leaf(leaf).is_none() {
            return;
        }
        self.focus = Some(leaf);
        if let Some(surface) = self.surface_of(leaf) {
            self.raise(surface);
        }
    }

    pub fn show(&mut self, tab: TabId) {
        let Some(position) = self.find(tab) else {
            return;
        };
        self.set_active_index(position.leaf, position.index);
        self.focus(position.leaf);
    }

    pub fn push(&mut self, leaf: LeafId, tab: TabId) {
        let index = self.tabs(leaf).len();
        self.insert(leaf, index, tab);
    }

    pub fn insert(&mut self, leaf: LeafId, index: usize, tab: TabId) {
        let Some(target) = self.leaf_mut(leaf) else {
            return;
        };
        let index = index.min(target.tabs.len());
        target.tabs.insert(index, tab);
        target.active = index;
        self.focus(leaf);
    }

    pub fn push_to_focused(&mut self, tab: TabId) {
        let leaf = self
            .focus
            .or_else(|| self.leaves(self.main()).first().copied());
        if let Some(leaf) = leaf {
            self.push(leaf, tab);
        }
    }

    pub fn split(
        &mut self,
        leaf: LeafId,
        side: Side,
        share: f32,
        tabs: Vec<TabId>,
    ) -> Option<LeafId> {
        let surface = self.surface_of(leaf)?;
        let added = self.new_leaf(tabs);
        let id = added.id;
        let split = SplitId(self.mint());
        let share = share.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION);
        let fraction = match side.leads() {
            true => share,
            false => 1.0 - share,
        };
        let direction = side.direction();
        let leads = side.leads();
        let root = &mut self.surface_mut(surface)?.root;
        root.replace_leaf(leaf, move |existing| {
            let added = Node::Leaf(added.clone());
            let (first, second) = match leads {
                true => (added, existing),
                false => (existing, added),
            };
            Node::Split(Split {
                id: split,
                direction,
                fraction,
                first: Box::new(first),
                second: Box::new(second),
            })
        });
        self.focus(id);
        Some(id)
    }

    pub fn remove(&mut self, tab: TabId) -> bool {
        let Some(position) = self.find(tab) else {
            return false;
        };
        let Some(leaf) = self.leaf_mut(position.leaf) else {
            return false;
        };
        leaf.tabs.remove(position.index);
        leaf.active = leaf.active.min(leaf.tabs.len().saturating_sub(1));
        if leaf.tabs.is_empty() {
            self.prune(position.leaf);
        }
        self.settle_focus();
        true
    }

    pub fn replace(&mut self, tab: TabId, with: TabId) -> bool {
        let Some(position) = self.find(tab) else {
            return false;
        };
        let Some(leaf) = self.leaf_mut(position.leaf) else {
            return false;
        };
        leaf.tabs[position.index] = with;
        true
    }

    pub fn drop_tab(&mut self, tab: TabId, target: DockDrop) {
        let Some(position) = self.find(tab) else {
            return;
        };
        let alone = self.tabs(position.leaf).len() == 1;
        match target {
            DockDrop::Tab { leaf, index } if leaf == position.leaf => {
                let Some(target) = self.leaf_mut(leaf) else {
                    return;
                };
                let index = index.min(target.tabs.len());
                let index = index - usize::from(index > position.index);
                target.tabs.remove(position.index);
                target.tabs.insert(index, tab);
                target.active = index;
                self.focus(leaf);
            }
            DockDrop::Tab { leaf, index } => {
                self.remove(tab);
                self.insert(leaf, index, tab);
            }
            DockDrop::Pane { leaf } => {
                if leaf == position.leaf && alone {
                    return;
                }
                self.remove(tab);
                self.push(leaf, tab);
            }
            DockDrop::Split { leaf, side } => {
                if leaf == position.leaf && alone {
                    return;
                }
                self.remove(tab);
                self.split(leaf, side, 0.5, vec![tab]);
            }
            DockDrop::Window { pos } => {
                let window = self
                    .surface_of(position.leaf)
                    .and_then(|surface| Some((surface, self.window_rect(surface)?)));
                if let Some((surface, rect)) = window
                    && alone
                {
                    self.set_window_rect(surface, Rect::from_min_size(pos, rect.size()));
                    return;
                }
                self.remove(tab);
                self.open_window(Rect::from_min_size(pos, FLOATING_SIZE), vec![tab]);
            }
        }
    }

    pub fn surface_tabs(&self, surface: SurfaceId) -> Vec<TabId> {
        let Some(surface) = self.surface(surface) else {
            return Vec::new();
        };
        let mut leaves = Vec::new();
        surface.root.walk(&mut leaves);
        leaves
            .into_iter()
            .flat_map(|leaf| leaf.tabs.iter().copied())
            .collect()
    }

    pub fn split_direction(&self, split: SplitId) -> Option<Direction> {
        self.surfaces
            .iter()
            .find_map(|surface| surface.root.split(split))
            .map(|split| split.direction)
    }

    pub fn split_fraction(&self, split: SplitId) -> f32 {
        self.surfaces
            .iter()
            .find_map(|surface| surface.root.split(split))
            .map_or(0.5, |split| split.fraction)
    }

    pub fn set_split_fraction(&mut self, split: SplitId, fraction: f32) {
        let fraction = fraction.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION);
        for surface in &mut self.surfaces {
            if let Some(found) = surface.root.split_mut(split) {
                found.fraction = fraction;
                return;
            }
        }
    }

    fn prune(&mut self, leaf: LeafId) {
        let Some(surface) = self.surface_of(leaf) else {
            return;
        };
        let main = self.main();
        let replacement = LeafId(self.mint());
        let Some(index) = self
            .surfaces
            .iter()
            .position(|candidate| candidate.id == surface)
        else {
            return;
        };
        let placeholder = Node::Leaf(Leaf {
            id: replacement,
            tabs: Vec::new(),
            active: 0,
        });
        let root = std::mem::replace(&mut self.surfaces[index].root, placeholder);
        match root.without_leaf(leaf) {
            Some(root) => self.surfaces[index].root = root,
            None if surface == main => {}
            None => {
                self.surfaces.remove(index);
            }
        }
    }

    fn settle_focus(&mut self) {
        if self.focus.is_some_and(|leaf| self.leaf(leaf).is_some()) {
            return;
        }
        self.focus = self.surfaces.last().and_then(|surface| {
            let mut leaves = Vec::new();
            surface.root.walk(&mut leaves);
            leaves.first().map(|leaf| leaf.id)
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockSplitter {
    pub id: SplitId,
    pub direction: Direction,
    pub handle: Rect,
    pub area: Rect,
    pub fraction: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockLayout {
    pub leaves: Vec<(LeafId, Rect)>,
    pub splitters: Vec<DockSplitter>,
}

impl DockLayout {
    pub fn leaf_rect(&self, leaf: LeafId) -> Option<Rect> {
        self.leaves
            .iter()
            .find_map(|(id, rect)| (*id == leaf).then_some(*rect))
    }

    pub fn leaf_at(&self, pos: Pos2) -> Option<LeafId> {
        self.leaves
            .iter()
            .find_map(|(id, rect)| rect.contains(pos).then_some(*id))
    }

    pub fn splitter(&self, split: SplitId) -> Option<DockSplitter> {
        self.splitters
            .iter()
            .find(|splitter| splitter.id == split)
            .copied()
    }
}

pub fn layout_surface(
    state: &DockState,
    surface: SurfaceId,
    area: Rect,
    thickness: f32,
) -> DockLayout {
    let mut layout = DockLayout::default();
    if let Some(surface) = state.surface(surface) {
        place(&surface.root, area, thickness, &mut layout);
    }
    layout
}

fn place(node: &Node, area: Rect, thickness: f32, out: &mut DockLayout) {
    match node {
        Node::Leaf(leaf) => out.leaves.push((leaf.id, area)),
        Node::Split(split) => {
            let (first, handle, second) = divide(area, split.direction, split.fraction, thickness);
            out.splitters.push(DockSplitter {
                id: split.id,
                direction: split.direction,
                handle,
                area,
                fraction: split.fraction,
            });
            place(&split.first, first, thickness, out);
            place(&split.second, second, thickness, out);
        }
    }
}

fn divide(area: Rect, direction: Direction, fraction: f32, thickness: f32) -> (Rect, Rect, Rect) {
    let available = (direction.main(area.size()) - thickness).max(0.0);
    let first = (available * fraction).round();
    match direction {
        Direction::Horizontal => {
            let split = area.left() + first;
            (
                Rect::from_min_max(area.min, Pos2::new(split, area.bottom())),
                Rect::from_min_max(
                    Pos2::new(split, area.top()),
                    Pos2::new(split + thickness, area.bottom()),
                ),
                Rect::from_min_max(Pos2::new(split + thickness, area.top()), area.max),
            )
        }
        Direction::Vertical => {
            let split = area.top() + first;
            (
                Rect::from_min_max(area.min, Pos2::new(area.right(), split)),
                Rect::from_min_max(
                    Pos2::new(area.left(), split),
                    Pos2::new(area.right(), split + thickness),
                ),
                Rect::from_min_max(Pos2::new(area.left(), split + thickness), area.max),
            )
        }
    }
}

pub fn fraction_moved(
    area: Rect,
    direction: Direction,
    thickness: f32,
    fraction: f32,
    moved: f32,
) -> f32 {
    let length = direction.main(area.size()) - thickness;
    if length <= 0.0 {
        return fraction;
    }
    (fraction + moved / length).clamp(MIN_FRACTION, 1.0 - MIN_FRACTION)
}
