use std::rc::Rc;

use beui::reactive::*;
use uuid::Uuid;

use be_block::Root;
use be_block::be_model::{Document, Field, FieldRef, List, Model, ObjectId};
use block_editor_plugin::Objects;

use crate::{ContentProjection, Editor};

pub struct RelatedContent<C> {
    editor: Editor,
    id: Memo<Option<Uuid>>,
    marker: std::marker::PhantomData<fn() -> C>,
}

impl<C> Clone for RelatedContent<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for RelatedContent<C> {}

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

    pub fn projection(&self) -> Option<ContentProjection<C>> {
        self.id().map(|id| self.editor.content_of::<C>(id))
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Option<T> {
        self.projection()?.read(read).ok()
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
        let read = create_signal(T::default());
        let id = self.id;
        let editor = self.editor;
        create_binding(move || {
            let Some(block) = id.get() else {
                read.set(T::default());
                return;
            };
            let project = Rc::clone(&project);
            let inner = editor
                .content_of::<C>(block)
                .project(move |content| project(content));
            create_binding(move || read.set(inner.get()));
        });
        read.read_only()
    }
}

impl<R: Root + 'static> RelatedContent<Document<R>>
where
    Document<R>: be_block::LiveEdit + Clone + Default,
{
    pub fn objects<M: 'static, T: Model + 'static>(
        &self,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
    ) -> ReadSignal<Objects<T>> {
        let read = create_signal(Objects::default());
        let id = self.id;
        let editor = self.editor;
        create_binding(move || {
            let Some(block) = id.get() else {
                read.set(Objects::default());
                return;
            };
            let inner = editor
                .content_of::<Document<R>>(block)
                .objects(owner, field);
            create_binding(move || read.set(inner.get()));
        });
        read.read_only()
    }

    pub fn field<M: 'static, F>(&self, object: ObjectId, field: FieldRef<M, F>) -> ReadSignal<F>
    where
        F: Field + Clone + Default + PartialEq + 'static,
    {
        let read = create_signal(F::default());
        let id = self.id;
        let editor = self.editor;
        create_binding(move || {
            let Some(block) = id.get() else {
                read.set(F::default());
                return;
            };
            let inner = editor.content_of::<Document<R>>(block).field(object, field);
            create_binding(move || read.set(inner.get()));
        });
        read.read_only()
    }
}
