use std::collections::HashMap;

use be_sdk_beui::be_block::BlockMetadata;
use be_sdk_beui::be_block::ObjectId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::{BlockList, BlockParent, BlockQuery, ChildTarget, ContentProjection, Editor};
use be_type_presentation::{Presentation, PresentationContent};
use uuid::Uuid;

/// The text block type, named by id so the deck does not depend on be-type-text: speaker notes
/// start as text, and any block may replace them.
pub const TEXT_TYPE: Uuid = Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_014b);

#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub target: Option<ChildTarget>,
    pub name: String,
    pub notes: Notes,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Notes {
    None,
    Loading,
    Block(ChildTarget),
}

#[derive(Clone, Copy)]
pub struct Slides {
    editor: Editor,
    block: ContentProjection<PresentationContent>,
    store: KeyedStore<Uuid, Slide>,
    selected: Signal<Option<Uuid>>,
    made_notes: StoredCell<Option<ChildTarget>>,
    focus_notes: Signal<Option<Uuid>>,
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
            made_notes: StoredCell::new(None),
            focus_notes: create_signal(None),
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

    pub fn add(&self, index: usize, anchor: impl Into<be_sdk_beui::PickAnchor>) {
        let block = self.block;
        let selected = self.selected;
        self.editor.pick_block(
            be_sdk_beui::BlockFilter {
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

    fn source_of(&self, id: Uuid) -> Option<Uuid> {
        self.block
            .read(|presentation| {
                presentation
                    .root()
                    .slides
                    .get(ObjectId::from_uuid(id))
                    .and_then(|slide| slide.block)
            })
            .ok()
            .flatten()
    }

    /// Copies the slide's block, and everything inside it, into the deck and shows the copy after
    /// the slide.
    pub fn duplicate(&self, id: Uuid) {
        let Some(source) = self.source_of(id) else {
            return;
        };
        let slides = *self;
        let deck = BlockParent::Block(self.editor.block_id());
        let asked = self.editor.host().copy_blocks(source, deck);
        self.editor.then(asked, move |copied| {
            let (Ok(Some((copy, _))), true) = (copied, slides.selected.is_alive()) else {
                return;
            };
            let index = slides
                .index_of_untracked(id)
                .map_or(usize::MAX, |index| index + 1);
            let slide = Uuid::new_v4();
            if let Ok(edit) = slides.block.read(|presentation| {
                presentation
                    .root()
                    .insert(ObjectId::from_uuid(slide), index, copy)
            }) {
                slides.block.operate(edit);
                slides.selected.set(Some(slide));
            }
        });
    }

    fn index_of_untracked(&self, id: Uuid) -> Option<usize> {
        self.store
            .keys()
            .with_untracked(|keys| keys.iter().position(|key| *key == id))
    }

    /// Makes an empty text block inside the deck as the slide's notes and asks for the keyboard
    /// to go into its editor once it is placed.
    pub fn add_notes(&self, id: Uuid) {
        let notes = self.editor.blocks().create_with(
            TEXT_TYPE,
            None,
            BlockParent::Block(self.editor.block_id()),
            BlockMetadata::default(),
        );
        self.made_notes
            .set(Some(ChildTarget::new(notes, TEXT_TYPE)));
        self.focus_notes.set(Some(notes));
        self.block.operate(Presentation::set_notes(
            ObjectId::from_uuid(id),
            Some(notes),
        ));
    }

    /// The notes block the keyboard should go into once its editor is placed.
    pub fn focus_notes(&self) -> Signal<Option<Uuid>> {
        self.focus_notes
    }

    fn refresh(&self, dependencies: &BlockList) {
        let Ok(entries) = self.block.read(|presentation| {
            presentation
                .root()
                .slides
                .iter()
                .map(|slide| (slide.id.as_uuid(), slide.block, slide.notes))
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
            .map(|(id, block, notes)| {
                let known = self.known(id);
                let notes = match notes {
                    None => Notes::None,
                    Some(notes) => metadata
                        .get(&notes)
                        .map(|reference| ChildTarget::new(reference.id, reference.block_type))
                        .or_else(|| {
                            let made = self.made_notes.get();
                            let kept = known.as_ref().and_then(|slide| match slide.notes {
                                Notes::Block(target) => Some(target),
                                _ => None,
                            });
                            made.into_iter()
                                .chain(kept)
                                .find(|target| target.id == notes)
                        })
                        .map_or(Notes::Loading, Notes::Block),
                };
                let reference = block.and_then(|id| metadata.get(&id));
                let slide = match reference {
                    Some(reference) => Slide {
                        target: Some(ChildTarget::new(reference.id, reference.block_type)),
                        name: reference.label(types.as_ref()).name,
                        notes,
                    },
                    None => known
                        .filter(|slide| {
                            slide.target.map(|target| target.id) == block && block.is_some()
                        })
                        .map(|slide| Slide { notes, ..slide })
                        .unwrap_or_else(|| Slide {
                            target: None,
                            name: match block {
                                Some(_) => pending.clone(),
                                None => "Broken link".to_owned(),
                            },
                            notes,
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
