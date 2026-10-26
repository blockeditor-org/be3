use block_editor_beui::beui::icons::Glyph;
use std::collections::{HashMap, HashSet};

use block_editor_beui::beui::Rect;
use block_editor_beui::beui::dock::styled::TabId;
use block_editor_beui::beui::icons::{ICON_BOXES, ICON_USERS};
use block_editor_beui::block_ui::{BlockCatalog, BlockTypes, TemplateCategory, TemplateEntry};
use block_editor_beui::sections::{is_workspace, shared_with_me, workspaces};
use block_editor_beui::{
    BlockFilter, BlockInfo, BlockParent, BlockPick, BlockQuery, ChildId, CreationProgress, Failure,
    PickRequest, PickedBlock, label_glyph,
};
use uuid::Uuid;

const SHARED: Uuid = Uuid::nil();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Menu,
    More,
    Link,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Creating {
    pub(crate) template: TemplateEntry,
    pub(crate) committed: bool,
    pub(crate) sent: bool,
    pub(crate) child: Option<ChildId>,
    pub(crate) height: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickAnchor {
    pub rect: Option<Rect>,
    pub tab: Option<TabId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TreeKey(pub(crate) Vec<Uuid>);

impl TreeKey {
    pub(crate) fn id(&self) -> Option<Uuid> {
        self.0.last().copied().filter(|id| *id != SHARED)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pick {
    pub(crate) pick: u64,
    pub(crate) filter: BlockFilter,
    pub(crate) parent: BlockParent,
    pub(crate) anchor: PickAnchor,
    pub(crate) touch: bool,
    pub(crate) stage: Stage,
    pub(crate) search: String,
    pub(crate) place: Option<BlockParent>,
    pub(crate) selected: Option<TreeKey>,
    pub(crate) unfolded: HashMap<TreeKey, bool>,
    pub(crate) unfolded_places: HashMap<TreeKey, bool>,
    pub(crate) creating: Option<Creating>,
    pub(crate) importing: Option<u64>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum PickAction {
    Stage(Stage),
    Search(String),
    Place(BlockParent),
    Make(TemplateEntry),
    Select(TreeKey),
    Unfold(TreeKey, bool),
    UnfoldPlace(TreeKey, bool),
    Link(Uuid, Uuid),
    Import,
    CopyTemplate(Uuid),
    Create,
    CancelCreation,
    Close,
    DismissError,
}

pub(crate) enum PickOutcome {
    Open(Box<Pick>),
    Answered(u64, BlockPick, Option<(Uuid, Uuid, Uuid)>),
}

impl Pick {
    pub(crate) fn new(request: PickRequest, tab: Option<TabId>) -> Self {
        let place = request
            .filter
            .place
            .map(BlockParent::decode)
            .and_then(|place| match place {
                BlockParent::Detached => None,
                place => Some(place),
            });
        Self {
            pick: request.pick,
            stage: Stage::Menu,
            filter: request.filter,
            parent: request.parent,
            anchor: PickAnchor {
                rect: request.anchor,
                tab,
            },
            touch: request.touch,
            search: String::new(),
            place,
            selected: None,
            unfolded: HashMap::new(),
            unfolded_places: HashMap::new(),
            creating: None,
            importing: None,
            error: None,
        }
    }

    pub(crate) fn choosing(&self) -> bool {
        self.creating.is_none() && self.error.is_none()
    }

    pub(crate) fn menu_open(&self) -> bool {
        self.choosing() && self.stage == Stage::Menu
    }

    pub(crate) fn surface_open(&self) -> bool {
        match &self.creating {
            Some(creating) => creating.template.dialog && self.error.is_none(),
            None => self.error.is_none() && self.stage != Stage::Menu,
        }
    }

    pub(crate) fn allowed(&self, block_type: Uuid) -> bool {
        self.filter.block_types.is_empty()
            || self.filter.block_types.contains(&block_type.into_bytes())
    }

    pub(crate) fn excluded(&self) -> HashSet<Uuid> {
        self.filter
            .excluded
            .iter()
            .copied()
            .map(Uuid::from_bytes)
            .collect()
    }

    pub(crate) fn created_parent(&self) -> BlockParent {
        match self.place {
            Some(BlockParent::Root) => BlockParent::Root,
            Some(BlockParent::Block(_) | BlockParent::Detached) | None => self.parent,
        }
    }

    pub(crate) fn import_parent(&self) -> BlockParent {
        match self.place {
            Some(BlockParent::Block(container)) => BlockParent::Block(container),
            _ => self.created_parent(),
        }
    }

    pub(crate) fn act(mut self, action: PickAction) -> PickOutcome {
        match action {
            PickAction::Stage(stage) => {
                if self.choosing() && self.importing.is_none() {
                    self.stage = stage;
                    self.search.clear();
                }
            }
            PickAction::Search(search) => self.search = search,
            PickAction::Place(place) => {
                if self.place.is_some() && self.importing.is_none() {
                    self.place = Some(place);
                }
            }
            PickAction::Select(key) => self.selected = Some(key),
            PickAction::Unfold(key, open) => {
                self.unfolded.insert(key, open);
            }
            PickAction::UnfoldPlace(key, open) => {
                self.unfolded_places.insert(key, open);
            }
            PickAction::Make(template) => {
                let dialog = template.dialog;
                self.creating = Some(Creating {
                    template,
                    committed: false,
                    sent: false,
                    child: None,
                    height: None,
                });
                if !dialog {
                    return self.act(PickAction::Create);
                }
            }
            PickAction::Link(id, block_type) => {
                return PickOutcome::Answered(
                    self.pick,
                    Ok(Some(PickedBlock {
                        block_id: id.into_bytes(),
                        block_type: block_type.into_bytes(),
                        linked: true,
                        placed: false,
                    })),
                    None,
                );
            }
            PickAction::Import | PickAction::CopyTemplate(_) => {}
            PickAction::Create => {
                if let Some(creating) = &mut self.creating {
                    creating.committed = true;
                }
            }
            PickAction::CancelCreation => {
                if self.creating.is_some() {
                    return PickOutcome::Answered(self.pick, Ok(None), None);
                }
            }
            PickAction::Close => {
                if self.choosing() {
                    return PickOutcome::Answered(self.pick, Ok(None), None);
                }
            }
            PickAction::DismissError => {
                return PickOutcome::Answered(self.pick, Ok(None), None);
            }
        }
        PickOutcome::Open(Box::new(self))
    }

    pub(crate) fn progressed(mut self, progress: &CreationProgress) -> PickOutcome {
        let Some(creating) = &self.creating else {
            return PickOutcome::Open(Box::new(self));
        };
        match progress {
            CreationProgress::Created(block) => {
                let block_type = creating.template.block_type;
                self.made(Uuid::from_bytes(*block), block_type)
            }
            CreationProgress::Failed(error) => {
                self.creating = None;
                self.error = Some(error.clone());
                PickOutcome::Open(Box::new(self))
            }
            CreationProgress::Options { .. } | CreationProgress::Working => {
                PickOutcome::Open(Box::new(self))
            }
        }
    }

    pub(crate) fn imported(
        mut self,
        imported: Result<Option<(Uuid, Uuid)>, Failure>,
    ) -> PickOutcome {
        self.importing = None;
        match imported {
            Ok(Some((block, block_type))) if self.allowed(block_type) => {
                self.made(block, block_type)
            }
            Ok(Some(_)) => {
                self.error = Some("The imported block can't be used here.".to_owned());
                PickOutcome::Open(Box::new(self))
            }
            Ok(None) => PickOutcome::Open(Box::new(self)),
            Err(error) => {
                self.error = Some(error.to_string());
                PickOutcome::Open(Box::new(self))
            }
        }
    }

    fn made(&self, block: Uuid, block_type: Uuid) -> PickOutcome {
        let into = match self.place {
            Some(BlockParent::Block(container)) => Some((block, block_type, container)),
            Some(BlockParent::Root | BlockParent::Detached) | None => None,
        };
        PickOutcome::Answered(
            self.pick,
            Ok(Some(PickedBlock {
                block_id: block.into_bytes(),
                block_type: block_type.into_bytes(),
                linked: false,
                placed: self.place.is_some(),
            })),
            into,
        )
    }
}

pub(crate) fn tile_key(template: &TemplateEntry) -> String {
    format!("{}/{}", template.editor, template.template)
}

fn offered<'a>(
    catalog: &'a BlockCatalog,
    pick: &'a Pick,
) -> impl Iterator<Item = &'a TemplateEntry> {
    catalog
        .templates()
        .iter()
        .filter(move |template| pick.allowed(template.block_type))
}

fn sorted(mut templates: Vec<TemplateEntry>) -> Vec<TemplateEntry> {
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    templates
}

pub(crate) fn common(catalog: &BlockCatalog, pick: &Pick) -> Vec<TemplateEntry> {
    sorted(
        offered(catalog, pick)
            .filter(|template| template.category == TemplateCategory::Important)
            .cloned()
            .collect(),
    )
}

pub(crate) fn slides(catalog: &BlockCatalog, pick: &Pick) -> Vec<TemplateEntry> {
    offered(catalog, pick)
        .filter(|template| template.category == TemplateCategory::Template)
        .cloned()
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TypeGroup {
    pub(crate) title: String,
    pub(crate) types: Vec<TemplateEntry>,
}

pub(crate) fn groups(catalog: &BlockCatalog, pick: &Pick) -> Vec<TypeGroup> {
    let query = pick.search.trim().to_lowercase();
    [
        (TemplateCategory::Important, "Common"),
        (TemplateCategory::Regular, "All blocks"),
        (TemplateCategory::Template, "Templates"),
        (TemplateCategory::Debug, "Debug"),
    ]
    .into_iter()
    .map(|(category, title)| TypeGroup {
        title: title.to_owned(),
        types: sorted(
            offered(catalog, pick)
                .filter(|template| template.category == category)
                .filter(|template| {
                    query.is_empty() || template.name.to_lowercase().contains(&query)
                })
                .cloned()
                .collect(),
        ),
    })
    .filter(|group| !group.types.is_empty())
    .collect()
}

pub(crate) struct Graph {
    pub(crate) organization: Uuid,
    pub(crate) account: Uuid,
    pub(crate) roots: Vec<BlockInfo>,
    pub(crate) shared: Vec<BlockInfo>,
    pub(crate) blocks: HashMap<Uuid, BlockInfo>,
    pub(crate) listed: HashSet<Uuid>,
}

impl Graph {
    fn sections(&self, types: &dyn BlockTypes) -> Vec<BlockInfo> {
        workspaces(self.roots.clone(), self.organization, self.account, types)
            .into_iter()
            .filter(|block| !block.deleted)
            .collect()
    }

    fn loose(&self) -> Vec<BlockInfo> {
        shared_with_me(self.shared.clone())
            .into_iter()
            .filter(|block| !block.deleted)
            .collect()
    }

    fn children(&self, block: &BlockInfo) -> impl Iterator<Item = &BlockInfo> {
        block
            .references
            .iter()
            .filter_map(|id| self.blocks.get(id))
            .filter(|child| !child.deleted)
    }

    fn ancestors(&self, block: Uuid) -> HashSet<Uuid> {
        let mut found = HashSet::new();
        let mut current = Some(block);
        while let Some(id) = current {
            if !found.insert(id) {
                break;
            }
            current = self
                .blocks
                .get(&id)
                .and_then(|info| info.parent)
                .and_then(BlockParent::block);
        }
        found
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PickTree {
    Link,
    Place,
}

pub(crate) struct Tree {
    pub(crate) rows: Vec<TreeRow>,
    pub(crate) wants: Vec<BlockQuery>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TreeRow {
    pub(crate) key: TreeKey,
    pub(crate) block_type: Uuid,
    pub(crate) name: String,
    pub(crate) icon: Option<Glyph>,
    pub(crate) detail: String,
    pub(crate) above: String,
    pub(crate) depth: usize,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) section: bool,
    pub(crate) enabled: bool,
}

struct Walk<'a> {
    catalog: &'a BlockCatalog,
    graph: &'a Graph,
    query: String,
    rows: Vec<TreeRow>,
    unfolded: Vec<Uuid>,
    shown: &'a dyn Fn(&BlockInfo) -> bool,
    enabled: &'a dyn Fn(&BlockInfo) -> bool,
    detail: &'a dyn Fn(&BlockInfo) -> String,
    open: &'a dyn Fn(&TreeKey, usize) -> bool,
}

impl Walk<'_> {
    fn matches(&self, block: &BlockInfo, name: &str) -> bool {
        self.query.is_empty()
            || name.to_lowercase().contains(&self.query)
            || block.id.to_string().contains(&self.query)
    }

    fn block(
        &mut self,
        block: &BlockInfo,
        path: &mut Vec<Uuid>,
        depth: usize,
        above: &str,
    ) -> bool {
        if path.contains(&block.id) || !(self.shown)(block) {
            return false;
        }
        let label = block.label(self.catalog);
        path.push(block.id);
        let key = TreeKey(path.clone());
        let searching = !self.query.is_empty();
        let unfolded = (self.open)(&key, depth);
        if unfolded {
            self.unfolded.push(block.id);
        }
        let expanded = searching || unfolded;
        let at = self.rows.len();
        let listed = self.graph.listed.contains(&block.id);
        let mut expandable = !listed && !block.references.is_empty();
        let mut kept_child = false;
        let children: Vec<BlockInfo> = match listed {
            true => self.graph.children(block).cloned().collect(),
            false => Vec::new(),
        };
        for child in &children {
            if path.contains(&child.id) || !(self.shown)(child) {
                continue;
            }
            expandable = true;
            if expanded {
                kept_child |= self.block(child, path, depth + 1, &label.name);
            }
        }
        path.pop();
        let matched = self.matches(block, &label.name);
        if !matched && !kept_child {
            self.rows.truncate(at);
            return false;
        }
        self.rows.insert(
            at,
            TreeRow {
                key,
                block_type: block.block_type,
                icon: label_glyph(&label),
                detail: (self.detail)(block),
                above: above.to_owned(),
                depth,
                expandable,
                expanded: expandable && expanded,
                section: depth == 0,
                enabled: (self.enabled)(block),
                name: label.name,
            },
        );
        true
    }

    fn shared(&mut self, loose: &[BlockInfo]) {
        let key = TreeKey(vec![SHARED]);
        let expanded = !self.query.is_empty() || (self.open)(&key, 0);
        let at = self.rows.len();
        let mut kept = false;
        let mut expandable = false;
        for block in loose {
            if !(self.shown)(block) {
                continue;
            }
            expandable = true;
            if expanded {
                kept |= self.block(block, &mut vec![SHARED], 1, "Shared with me");
            }
        }
        if !expandable || (!self.query.is_empty() && !kept) {
            return;
        }
        self.rows.insert(
            at,
            TreeRow {
                key,
                block_type: Uuid::nil(),
                name: "Shared with me".to_owned(),
                icon: Some(ICON_USERS),
                detail: String::new(),
                above: String::new(),
                depth: 0,
                expandable,
                expanded,
                section: true,
                enabled: false,
            },
        );
    }
}

fn section_icon(row: &mut TreeRow) {
    if row.icon.is_none() {
        row.icon = Some(ICON_BOXES);
    }
}

fn tree(walk: Walk, anchor: Option<Uuid>) -> Tree {
    let mut rows = walk.rows;
    rows.iter_mut()
        .filter(|row| row.section)
        .for_each(section_icon);
    let mut wants: Vec<BlockQuery> = walk
        .unfolded
        .into_iter()
        .map(BlockQuery::Children)
        .collect();
    if let Some(anchor) = anchor {
        wants.extend([BlockQuery::Block(anchor), BlockQuery::Parents(anchor)]);
    }
    wants.sort();
    wants.dedup();
    Tree { rows, wants }
}

pub(crate) fn link_rows(catalog: &BlockCatalog, pick: &Pick, graph: &Graph) -> Tree {
    let excluded = pick.excluded();
    let this = pick.parent.block();
    let ancestors = this.map(|this| graph.ancestors(this)).unwrap_or_default();
    let shown = |block: &BlockInfo| !block.deleted;
    let enabled = |block: &BlockInfo| {
        block.access.can_view()
            && !is_workspace(block)
            && block
                .parent
                .is_some_and(|parent| parent != BlockParent::Detached)
            && !excluded.contains(&block.id)
            && !ancestors.contains(&block.id)
            && pick.allowed(block.block_type)
    };
    let detail = |block: &BlockInfo| match Some(block.id) == this {
        true => "This block".to_owned(),
        false if is_workspace(block) => String::new(),
        false => catalog
            .display_name(block.block_type)
            .unwrap_or_default()
            .to_owned(),
    };
    let mut walk = Walk {
        catalog,
        graph,
        query: pick.search.trim().to_lowercase(),
        rows: Vec::new(),
        unfolded: Vec::new(),
        shown: &shown,
        enabled: &enabled,
        detail: &detail,
        open: &|key: &TreeKey, depth: usize| pick.unfolded.get(key).copied().unwrap_or(depth == 0),
    };
    for section in graph.sections(catalog) {
        walk.block(&section, &mut Vec::new(), 0, "");
    }
    walk.shared(&graph.loose());
    tree(walk, this)
}

pub(crate) fn place_rows(catalog: &BlockCatalog, pick: &Pick, graph: &Graph) -> Tree {
    let Some(place) = pick.place else {
        return Tree {
            rows: Vec::new(),
            wants: Vec::new(),
        };
    };
    let excluded = pick.excluded();
    let current = place.block();
    let shown = |block: &BlockInfo| {
        let held = Some(block.id) == current;
        !block.deleted
            && (held
                || (block.access.can_edit()
                    && (catalog.child_edits(block.block_type).add || is_workspace(block))
                    && !excluded.contains(&block.id)))
    };
    let enabled = |_: &BlockInfo| true;
    let detail = |_: &BlockInfo| String::new();
    let held = place_path(graph, place);
    let open = |key: &TreeKey, depth: usize| {
        pick.unfolded_places
            .get(key)
            .copied()
            .unwrap_or_else(|| depth == 0 || held.starts_with(&key.0))
    };
    let mut walk = Walk {
        catalog,
        graph,
        query: String::new(),
        rows: Vec::new(),
        unfolded: Vec::new(),
        shown: &shown,
        enabled: &enabled,
        detail: &detail,
        open: &open,
    };
    for section in graph.sections(catalog) {
        walk.block(&section, &mut Vec::new(), 0, "");
    }
    for block in graph.loose() {
        walk.block(&block, &mut Vec::new(), 0, "");
    }
    tree(walk, current)
}

pub(crate) fn place_path(graph: &Graph, place: BlockParent) -> Vec<Uuid> {
    let Some(block) = place.block() else {
        return Vec::new();
    };
    let mut path = Vec::new();
    let mut current = Some(block);
    while let Some(id) = current {
        if path.contains(&id) {
            break;
        }
        path.push(id);
        current = graph
            .blocks
            .get(&id)
            .and_then(|info| info.parent)
            .and_then(BlockParent::block);
    }
    path.reverse();
    path
}
