use std::collections::HashSet;

use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Presentation {
    pub slides: List<Slide>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Slide {
    pub block: Option<Uuid>,
}

impl Presentation {
    fn anchor_at(&self, index: usize, skipping: Option<ObjectId>) -> Anchor {
        let before: Vec<ObjectId> = self
            .slides
            .iter()
            .map(|slide| slide.id)
            .filter(|id| Some(*id) != skipping)
            .take(index)
            .collect();
        before.last().map_or(Anchor::Start, |id| Anchor::After(*id))
    }

    pub fn insert(&self, slide: ObjectId, index: usize, block: Uuid) -> Edit {
        Self::SLIDES
            .insert_as(
                slide,
                ObjectId::ROOT,
                self.anchor_at(index, None),
                &Slide { block: Some(block) },
            )
            .into()
    }

    pub fn remove(slide: ObjectId) -> Edit {
        Change::remove(slide).into()
    }

    pub fn move_to(&self, slide: ObjectId, index: usize) -> Edit {
        Self::SLIDES
            .move_into(ObjectId::ROOT, self.anchor_at(index, Some(slide)), slide)
            .into()
    }

    pub fn set_block(slide: ObjectId, block: Uuid) -> Edit {
        Slide::BLOCK.set(slide, &Some(block)).into()
    }
}

impl Root for Presentation {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7072_6573_656e_7461_7469_6f6e_2d63_0001);

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.slides
            .iter()
            .filter_map(|slide| Some(slide.block?))
            .filter(|block| seen.insert(*block))
            .collect()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let showing = |block: Uuid| {
            self.slides
                .iter()
                .filter(move |slide| slide.block == Some(block))
        };
        Some(match change {
            ChildChange::Add(block) => self.insert(ObjectId::new(), self.slides.len(), block),
            ChildChange::Delete(block) => showing(block)
                .map(|slide| Change::remove(slide.id))
                .collect(),
            ChildChange::Replace { old, new } => showing(old)
                .flat_map(|slide| Self::set_block(slide.id, new).0)
                .collect(),
        })
    }
}

pub type PresentationContent = Document<Presentation>;
