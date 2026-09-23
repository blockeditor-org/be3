use be_commit::MergeResult;
use be_model::{Document, Edit, Model, Step, Touched};
use uuid::Uuid;

use crate::{BlockContent, ChildChange, ContentError, LiveEdit, Merge, Undo};

pub trait Root: Model + Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    fn name(&self) -> Option<String> {
        None
    }

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let _ = change;
        None
    }
}

impl<R: Root> BlockContent for Document<R> {
    const CONTENT_TYPE: Uuid = R::CONTENT_TYPE;

    fn encode(&self) -> Vec<u8> {
        self.to_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        Document::from_bytes(bytes).map_err(|_| ContentError::Malformed("model document"))
    }

    fn name(&self) -> Option<String> {
        self.root().name()
    }

    fn references(&self) -> Vec<Uuid> {
        self.root().references()
    }
}

impl<R: Root> LiveEdit for Document<R> {
    type Op = Edit;

    fn apply(&mut self, operation: &Self::Op) {
        Document::apply(self, operation);
    }

    fn apply_touching(&mut self, operation: &Self::Op, touched: &mut Vec<Touched>) {
        Document::apply_touching(self, operation, touched);
    }

    fn child_operations(&self, change: ChildChange) -> Option<Vec<Self::Op>> {
        self.root().child_edit(change).map(|edit| vec![edit])
    }
}

impl<R: Root> Merge for Document<R> {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        match Document::merge(base, ours, theirs) {
            (value, 0) => MergeResult::Clean(value),
            (value, conflicts) => MergeResult::Conflicted { value, conflicts },
        }
    }
}

impl<R: Root> Undo for Document<R> {
    type Step = Step;

    fn step(&self, operation: &Self::Op) -> Option<Self::Step> {
        Document::step(self, operation)
    }

    fn absorb(previous: &mut Self::Step, next: Self::Step) -> Result<(), Self::Step> {
        previous.absorb(next)
    }

    fn revert(&self, step: &Self::Step) -> Vec<Self::Op> {
        vec![step.undo()]
    }

    fn reapply(&self, step: &Self::Step) -> Vec<Self::Op> {
        vec![step.redo()]
    }
}
