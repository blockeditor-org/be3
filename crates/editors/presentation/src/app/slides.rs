use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::presentation::{Presentation, PresentationContent};
use block_editor_beui::beui::reactive::{KeyedStore, ReadSignal, WriteSignal, create_signal};
use block_editor_beui::{BlockList, BlockQuery, ChildTarget, ContentProjection, Editor};
use uuid::Uuid;

struct PendingSlide {
    block: Uuid,
    slide: Uuid,
    index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub target: Option<ChildTarget>,
    pub name: String,
}

pub struct Slides {
    editor: Editor,
    block: Rc<ContentProjection<PresentationContent>>,
    store: KeyedStore<Uuid, Slide>,
    selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
    pending: Rc<RefCell<Vec<PendingSlide>>>,
}

impl Slides {
    pub fn new(editor: &Editor) -> Rc<Self> {
        let block = editor.block_content::<PresentationContent>();
        let (selected, set_selected) = create_signal(None::<Uuid>);
        let slides = Rc::new(Self {
            editor: editor.clone(),
            block,
            store: KeyedStore::new(),
            selected,
            set_selected,
            pending: Rc::new(RefCell::new(Vec::new())),
        });
        let dependencies = editor
            .blocks()
            .watch(BlockQuery::References(editor.block_id()));
        let updated = Rc::clone(&slides);
        editor.each_frame(move || updated.refresh(&dependencies));
        slides
    }

    pub fn keys(&self) -> ReadSignal<Vec<Uuid>> {
        self.store.keys()
    }

    pub fn selected(&self) -> ReadSignal<Option<Uuid>> {
        self.selected.clone()
    }

    pub fn select(&self, id: Option<Uuid>) {
        self.set_selected.set(id);
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
        self.set_selected.set(Some(keys[next]));
    }

    pub fn go_to(&self, index: usize) {
        let keys = self.store.keys().get_untracked();
        if let Some(id) = keys.get(index.min(keys.len().saturating_sub(1))) {
            self.set_selected.set(Some(*id));
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
            self.set_selected.set(next.copied());
        }
    }

    pub fn move_to(&self, id: Uuid, index: usize) {
        if let Some(edit) = self
            .block
            .read(|presentation| presentation.root().move_to(ObjectId::from_uuid(id), index))
        {
            self.block.operate(edit);
        }
    }

    pub fn add(self: &Rc<Self>, index: usize) {
        let slides = Rc::clone(self);
        self.editor.pick_block(
            block_editor_beui::BlockFilter {
                name: "Slide".into(),
                block_types: Vec::new(),
                excluded: Vec::new(),
                templates: true,
            },
            move |picked| {
                let Ok(picked) = picked else {
                    return;
                };
                let slide_id = Uuid::new_v4();
                slides.pending.borrow_mut().push(PendingSlide {
                    block: picked.id,
                    slide: slide_id,
                    index,
                });
                slides.set_selected.set(Some(slide_id));
            },
        );
    }

    fn refresh(&self, dependencies: &BlockList) {
        let inserts: Vec<_> = std::mem::take(&mut *self.pending.borrow_mut());
        for PendingSlide {
            block,
            slide,
            index,
        } in inserts
        {
            if let Some(edit) = self.block.read(|presentation| {
                presentation
                    .root()
                    .insert(ObjectId::from_uuid(slide), index, block)
            }) {
                self.block.operate(edit);
            }
        }
        let Some(entries) = self.block.read(|presentation| {
            presentation
                .root()
                .slides
                .iter()
                .map(|slide| (slide.id.as_uuid(), slide.block))
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        let metadata: HashMap<_, _> = dependencies
            .read()
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
                                Some(_) => "Loading…".to_owned(),
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
            self.set_selected.set(keys.first().copied());
        }
    }
}
