use std::collections::HashMap;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{BlockList, BlockQuery, ChildTarget, ContentProjection, Editor};
use presentation_types::{Presentation, PresentationContent};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub target: Option<ChildTarget>,
    pub name: String,
}

#[derive(Clone, Copy)]
pub struct Slides {
    editor: Editor,
    block: ContentProjection<PresentationContent>,
    store: KeyedStore<Uuid, Slide>,
    selected: Signal<Option<Uuid>>,
}

impl Slides {
    pub fn new(editor: Editor) -> Self {
        let block = editor.block_content::<PresentationContent>();
        let selected = create_signal(None::<Uuid>);
        let slides = Self {
            editor,
            block,
            store: KeyedStore::new(),
            selected,
        };
        let dependencies = editor
            .blocks()
            .watch(BlockQuery::References(editor.block_id()));
        create_binding(move || slides.refresh(&dependencies));
        slides
    }

    pub fn keys(&self) -> ReadSignal<Vec<Uuid>> {
        self.store.keys()
    }

    pub fn selected(&self) -> ReadSignal<Option<Uuid>> {
        self.selected.read_only()
    }

    pub fn select(&self, id: Option<Uuid>) {
        self.selected.set(id);
    }

    pub fn slide(&self, id: Uuid) -> Option<Slide> {
        let known = self.store.keys().with(|keys| keys.contains(&id));
        known.then(|| self.store.get(&id).get())
    }

    fn known(&self, id: Uuid) -> Option<Slide> {
        let known = self.store.keys().with_untracked(|keys| keys.contains(&id));
        known.then(|| self.store.get(&id).get_untracked())
    }

    pub fn target(&self, id: Option<Uuid>) -> Option<ChildTarget> {
        self.slide(id?)?.target
    }

    pub fn index_of(&self, id: Uuid) -> Option<usize> {
        self.store
            .keys()
            .with(|keys| keys.iter().position(|key| *key == id))
    }

    pub fn count(&self) -> usize {
        self.store.keys().with(Vec::len)
    }

    pub fn step(&self, offset: isize) {
        let keys = self.store.keys().get_untracked();
        if keys.is_empty() {
            return;
        }
        let current = self
            .selected
            .get_untracked()
            .and_then(|id| keys.iter().position(|key| *key == id))
            .unwrap_or(0);
        let next = current
            .saturating_add_signed(offset)
            .min(keys.len().saturating_sub(1));
        self.selected.set(Some(keys[next]));
    }

    pub fn go_to(&self, index: usize) {
        let keys = self.store.keys().get_untracked();
        if let Some(id) = keys.get(index.min(keys.len().saturating_sub(1))) {
            self.selected.set(Some(*id));
        }
    }

    pub fn remove(&self, id: Uuid) {
        let index = self.index_of(id);
        self.block
            .operate(Presentation::remove(ObjectId::from_uuid(id)));
        if self.selected.get_untracked() == Some(id) {
            let keys = self.store.keys().get_untracked();
            let next = index.and_then(|index| {
                keys.get(index + 1)
                    .or_else(|| keys.get(index.wrapping_sub(1)))
            });
            self.selected.set(next.copied());
        }
    }

    pub fn move_to(&self, id: Uuid, index: usize) {
        if let Ok(edit) = self
            .block
            .read(|presentation| presentation.root().move_to(ObjectId::from_uuid(id), index))
        {
            self.block.operate(edit);
        }
    }

    pub fn add(&self, index: usize, anchor: impl Into<block_editor_beui::PickAnchor>) {
        let block = self.block;
        let selected = self.selected;
        self.editor.pick_block(
            block_editor_beui::BlockFilter {
                name: "Slide".into(),
                block_types: Vec::new(),
                excluded: Vec::new(),
                templates: true,
                place: None,
            },
            anchor,
            move |picked| {
                let Ok(picked) = picked else {
                    return;
                };
                let slide = Uuid::new_v4();
                if let Ok(edit) = block.read(|presentation| {
                    presentation
                        .root()
                        .insert(ObjectId::from_uuid(slide), index, picked.id())
                }) {
                    block.operate(edit);
                }
                selected.set(Some(slide));
            },
        );
    }

    fn refresh(&self, dependencies: &BlockList) {
        let Ok(entries) = self.block.read(|presentation| {
            presentation
                .root()
                .slides
                .iter()
                .map(|slide| (slide.id.as_uuid(), slide.block))
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        let references = dependencies.read();
        let pending = match &references {
            Err(Pending::Failed(error)) => error.clone(),
            _ => "Loading…".to_owned(),
        };
        let metadata: HashMap<_, _> = references
            .unwrap_or_default()
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect();
        let types = self.editor.block_types();
        let items: Vec<_> = entries
            .into_iter()
            .map(|(id, block)| {
                let reference = block.and_then(|id| metadata.get(&id));
                let slide = match reference {
                    Some(reference) => Slide {
                        target: Some(ChildTarget::new(reference.id, reference.block_type)),
                        name: reference.label(types.as_ref()).name,
                    },
                    None => self
                        .known(id)
                        .filter(|slide| {
                            slide.target.map(|target| target.id) == block && block.is_some()
                        })
                        .unwrap_or_else(|| Slide {
                            target: None,
                            name: match block {
                                Some(_) => pending.clone(),
                                None => "Broken link".to_owned(),
                            },
                        }),
                };
                (id, slide)
            })
            .collect();
        self.store.reconcile_owned(items);
        let keys = self.store.keys().get_untracked();
        let selected = self.selected.get_untracked();
        if selected.is_none_or(|selected| !keys.contains(&selected)) {
            self.selected.set(keys.first().copied());
        }
    }
}
