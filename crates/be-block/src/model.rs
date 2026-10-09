use be_commit::MergeResult;
use be_model::{Change, Document, Edit, Model, Step, Touched};
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

    fn references_in(&self, workspace: Uuid) -> Vec<Uuid> {
        let _ = workspace;
        self.references()
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
        joined(self.root().references(), self.block_refs())
    }

    fn references_in(&self, workspace: Uuid) -> Vec<Uuid> {
        joined(self.root().references_in(workspace), self.block_refs())
    }
}

fn joined(mut references: Vec<Uuid>, more: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen: std::collections::HashSet<Uuid> = references.iter().copied().collect();
    references.extend(more.into_iter().filter(|id| seen.insert(*id)));
    references
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

    fn absorb_operation(operation: &mut Self::Op, next: Self::Op) -> Option<Self::Op> {
        for change in next.0 {
            let leftover = match (operation.0.last_mut(), change) {
                (
                    Some(Change::Text { object, field, op }),
                    Change::Text {
                        object: next_object,
                        field: next_field,
                        op: next_op,
                    },
                ) if *object == next_object && *field == next_field => {
                    op.absorb(next_op).map(|op| Change::Text {
                        object: next_object,
                        field: next_field,
                        op,
                    })
                }
                (_, change) => Some(change),
            };
            operation.0.extend(leftover);
        }
        None
    }

    fn session_state(&self) -> Vec<u8> {
        Document::session_state(self)
    }

    fn adopt_session_state(&mut self, state: &[u8]) -> Result<(), ContentError> {
        Document::adopt_session_state(self, state)
            .map_err(|_| ContentError::Malformed("session state"))
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
