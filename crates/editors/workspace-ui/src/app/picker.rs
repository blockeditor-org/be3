use std::collections::HashSet;

use block_editor_beui::be_block::{BlockContent, FolderContent};
use block_editor_beui::block_ui::{BlockCatalog, BlockTypes, TemplateCategory, TemplateEntry};
use block_editor_beui::{
    BlockFilter, BlockInfo, BlockParent, BlockPick, CreationProgress, PickRequest,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PickerTab {
    Add,
    Templates,
    LinkExisting,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Creating {
    pub(crate) template: TemplateEntry,
    pub(crate) committed: bool,
    pub(crate) sent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pick {
    pub(crate) pick: u64,
    pub(crate) filter: BlockFilter,
    pub(crate) parent: BlockParent,
    pub(crate) tab: PickerTab,
    pub(crate) search: String,
    pub(crate) name: String,
    pub(crate) place: Option<BlockParent>,
    pub(crate) creating: Option<Creating>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum PickAction {
    Tab(PickerTab),
    Search(String),
    Name(String),
    Place(BlockParent),
    Make(TemplateEntry),
    Link(Uuid, Uuid),
    Create,
    CancelCreation,
    Close,
    DismissError,
}

pub(crate) enum PickOutcome {
    Open(Pick),
    Answered(u64, BlockPick, Option<(Uuid, Uuid, Uuid)>),
}

impl Pick {
    pub(crate) fn new(request: PickRequest) -> Self {
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
            tab: match request.filter.templates {
                true => PickerTab::Templates,
                false => PickerTab::Add,
            },
            filter: request.filter,
            parent: request.parent,
            search: String::new(),
            name: String::new(),
            place,
            creating: None,
            error: None,
        }
    }

    pub(crate) fn choosing(&self) -> bool {
        self.creating.is_none() && self.error.is_none()
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

    pub(crate) fn act(mut self, action: PickAction) -> PickOutcome {
        match action {
            PickAction::Tab(tab) => self.tab = tab,
            PickAction::Search(search) => self.search = search,
            PickAction::Name(name) => self.name = name,
            PickAction::Place(place) => {
                if self.place.is_some() {
                    self.place = Some(place);
                }
            }
            PickAction::Make(template) => {
                let dialog = template.dialog;
                self.creating = Some(Creating {
                    template,
                    committed: false,
                    sent: false,
                });
                if !dialog {
                    return self.act(PickAction::Create);
                }
            }
            PickAction::Link(id, block_type) => {
                return PickOutcome::Answered(
                    self.pick,
                    BlockPick::Chosen {
                        block_id: id.into_bytes(),
                        block_type: block_type.into_bytes(),
                        linked: true,
                        placed: false,
                    },
                    None,
                );
            }
            PickAction::Create => {
                if let Some(creating) = &mut self.creating {
                    creating.committed = true;
                }
            }
            PickAction::CancelCreation => {
                if self.creating.is_some() {
                    return PickOutcome::Answered(self.pick, BlockPick::Cancelled, None);
                }
            }
            PickAction::Close => {
                if self.choosing() {
                    return PickOutcome::Answered(self.pick, BlockPick::Cancelled, None);
                }
            }
            PickAction::DismissError => {
                return PickOutcome::Answered(self.pick, BlockPick::Cancelled, None);
            }
        }
        PickOutcome::Open(self)
    }

    pub(crate) fn progressed(mut self, progress: &CreationProgress) -> PickOutcome {
        let Some(creating) = &self.creating else {
            return PickOutcome::Open(self);
        };
        match progress {
            CreationProgress::Created(block) => {
                let block_type = creating.template.block_type;
                let placed = self.place.is_some();
                let into = match self.place {
                    Some(BlockParent::Block(container)) => {
                        Some((Uuid::from_bytes(*block), block_type, container))
                    }
                    Some(BlockParent::Root | BlockParent::Detached) | None => None,
                };
                PickOutcome::Answered(
                    self.pick,
                    BlockPick::Chosen {
                        block_id: *block,
                        block_type: block_type.into_bytes(),
                        linked: false,
                        placed,
                    },
                    into,
                )
            }
            CreationProgress::Failed(error) => {
                self.creating = None;
                self.error = Some(error.clone());
                PickOutcome::Open(self)
            }
            CreationProgress::Options { .. } | CreationProgress::Working => PickOutcome::Open(self),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TileSection {
    pub(crate) key: String,
    pub(crate) title: String,
    pub(crate) icon: Option<String>,
    pub(crate) tiles: Vec<TemplateEntry>,
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

pub(crate) fn sections(catalog: &BlockCatalog, pick: &Pick) -> Vec<TileSection> {
    match pick.tab {
        PickerTab::Add => add_sections(catalog, pick),
        PickerTab::Templates => template_sections(catalog, pick),
        PickerTab::LinkExisting => Vec::new(),
    }
}

fn add_sections(catalog: &BlockCatalog, pick: &Pick) -> Vec<TileSection> {
    [
        (TemplateCategory::Important, "Common"),
        (TemplateCategory::Regular, "More blocks"),
        (TemplateCategory::Debug, "Debug"),
    ]
    .into_iter()
    .map(|(category, title)| {
        let mut tiles: Vec<TemplateEntry> = offered(catalog, pick)
            .filter(|template| template.category == category)
            .cloned()
            .collect();
        tiles.sort_by(|a, b| a.name.cmp(&b.name));
        TileSection {
            key: format!("{category:?}"),
            title: title.to_owned(),
            icon: None,
            tiles,
        }
    })
    .filter(|section| !section.tiles.is_empty())
    .collect()
}

fn template_sections(catalog: &BlockCatalog, pick: &Pick) -> Vec<TileSection> {
    let mut sections: Vec<TileSection> = Vec::new();
    for template in offered(catalog, pick) {
        if template.category != TemplateCategory::Template {
            continue;
        }
        let key = template.editor.to_string();
        match sections.iter_mut().find(|section| section.key == key) {
            Some(section) => section.tiles.push(template.clone()),
            None => sections.push(TileSection {
                key,
                title: catalog
                    .display_name(template.editor)
                    .unwrap_or_default()
                    .to_owned(),
                icon: catalog.icon(template.editor).map(str::to_owned),
                tiles: vec![template.clone()],
            }),
        }
    }
    sections.sort_by(|a, b| a.title.cmp(&b.title));
    sections
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LinkRow {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) name: String,
    pub(crate) icon: String,
}

pub(crate) fn links(catalog: &BlockCatalog, pick: &Pick, blocks: &[BlockInfo]) -> Vec<LinkRow> {
    let excluded = pick.excluded();
    let query = pick.search.trim().to_lowercase();
    blocks
        .iter()
        .filter(|block| block.access.can_view())
        .filter(|block| block.parent != BlockParent::Detached)
        .filter(|block| !excluded.contains(&block.id))
        .filter(|block| pick.allowed(block.block_type))
        .map(|block| (block.label(catalog), block.id, block.block_type))
        .filter(|(label, id, _)| {
            query.is_empty()
                || label.name.to_lowercase().contains(&query)
                || id.to_string().contains(&query)
        })
        .map(|(label, id, block_type)| LinkRow {
            id,
            block_type,
            name: label.name,
            icon: label.icon.unwrap_or_default().to_owned(),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Place {
    pub(crate) parent: BlockParent,
    pub(crate) name: String,
    pub(crate) icon: String,
}

pub(crate) fn places(catalog: &BlockCatalog, pick: &Pick, blocks: &[BlockInfo]) -> Vec<Place> {
    let Some(place) = pick.place else {
        return Vec::new();
    };
    let excluded = pick.excluded();
    let folder_type = FolderContent::CONTENT_TYPE;
    let folder = catalog.icon(folder_type).unwrap_or_default().to_owned();
    let mut folders: Vec<Place> = blocks
        .iter()
        .filter(|block| block.access.can_edit())
        .filter(|block| block.parent != BlockParent::Detached)
        .filter(|block| block.block_type == folder_type || place == BlockParent::Block(block.id))
        .filter(|block| !excluded.contains(&block.id) || place == BlockParent::Block(block.id))
        .map(|block| {
            let label = block.label(catalog);
            Place {
                parent: BlockParent::Block(block.id),
                name: label.name,
                icon: label.icon.map_or_else(|| folder.clone(), str::to_owned),
            }
        })
        .collect();
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    std::iter::once(Place {
        parent: BlockParent::Root,
        name: "Top level".to_owned(),
        icon: folder,
    })
    .chain(folders)
    .collect()
}
