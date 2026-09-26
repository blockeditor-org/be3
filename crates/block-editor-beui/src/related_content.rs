use std::rc::Rc;

use beui::reactive::{Memo, ReadSignal, create_effect, create_signal};
use uuid::Uuid;

use crate::{ContentProjection, Editor};

pub struct RelatedContent<C> {
    editor: Editor,
    id: Memo<Option<Uuid>>,
    marker: std::marker::PhantomData<fn() -> C>,
}

impl<C> RelatedContent<C>
where
    C: be_block::LiveEdit + Clone + Default,
{
    pub(crate) fn new(editor: Editor, id: Memo<Option<Uuid>>) -> Self {
        Self {
            editor,
            id,
            marker: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> Option<Uuid> {
        self.id.get_untracked()
    }

    pub fn projection(&self) -> Option<Rc<ContentProjection<C>>> {
        self.id().map(|id| self.editor.content_of::<C>(id))
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Option<T> {
        self.projection()?.read(read)
    }

    pub fn operate(&self, operation: C::Op) {
        if let Some(projection) = self.projection() {
            projection.operate(operation);
        }
    }

    pub fn project<T>(&self, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + Default + PartialEq + 'static,
    {
        let project = Rc::new(project);
        let (read, write) = create_signal(T::default());
        let editor = self.editor.clone();
        let id = self.id.clone();
        create_effect(move || {
            let Some(block) = id.get() else {
                write.set(T::default());
                return;
            };
            let project = Rc::clone(&project);
            let inner = editor
                .content_of::<C>(block)
                .project(move |content| project(content));
            let write = write.clone();
            create_effect(move || write.set(inner.get()));
        });
        read
    }
}
